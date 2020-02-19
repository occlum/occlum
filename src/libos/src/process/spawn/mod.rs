use super::*;

use std::ffi::{CStr, CString};
use std::path::Path;
use std::sgxfs::SgxFile;

use super::fs::{
    CreationFlags, File, FileDesc, FileTable, INodeExt, StdinFile, StdoutFile, ROOT_INODE,
};
use super::misc::ResourceLimitsRef;
use super::vm::{ProcessVM, ProcessVMBuilder};

pub use self::elf_file::{ElfFile, ProgramHeaderExt};
use self::init_stack::{AuxKey, AuxTable};

mod elf_file;
mod init_stack;
mod init_vm;

pub fn do_spawn(
    elf_path: &str,
    argv: &[CString],
    envp: &[CString],
    file_actions: &[FileAction],
    parent_ref: &ProcessRef,
) -> Result<pid_t> {
    let (new_tid, new_process_ref) = new_process(elf_path, argv, envp, file_actions, parent_ref)?;
    task::enqueue_and_exec_task(new_tid, new_process_ref);
    Ok(new_tid)
}

pub fn do_spawn_without_exec(
    elf_path: &str,
    argv: &[CString],
    envp: &[CString],
    file_actions: &[FileAction],
    parent_ref: &ProcessRef,
) -> Result<pid_t> {
    let (new_tid, new_process_ref) = new_process(elf_path, argv, envp, file_actions, parent_ref)?;
    task::enqueue_task(new_tid, new_process_ref);
    Ok(new_tid)
}

fn new_process(
    elf_path: &str,
    argv: &[CString],
    envp: &[CString],
    file_actions: &[FileAction],
    parent_ref: &ProcessRef,
) -> Result<(pid_t, ProcessRef)> {
    let elf_buf = load_elf_to_vec(elf_path, parent_ref)
        .cause_err(|e| errno!(e.errno(), "cannot load the executable"))?;
    let ldso_path = "/lib/ld-musl-x86_64.so.1";
    let ldso_elf_buf = load_elf_to_vec(ldso_path, parent_ref)
        .cause_err(|e| errno!(e.errno(), "cannot load ld.so"))?;

    let exec_elf_file =
        ElfFile::new(&elf_buf).cause_err(|e| errno!(e.errno(), "invalid executable"))?;
    let ldso_elf_file =
        ElfFile::new(&ldso_elf_buf).cause_err(|e| errno!(e.errno(), "invalid ld.so"))?;

    let (new_pid, new_process_ref) = {
        let cwd = parent_ref.lock().unwrap().get_cwd().to_owned();
        let vm = init_vm::do_init(&exec_elf_file, &ldso_elf_file)?;
        let auxtbl = init_auxtbl(&vm, &exec_elf_file)?;

        // Notify debugger to load the symbols from elf file
        let ldso_elf_base = vm.get_elf_ranges()[1].start() as u64;
        unsafe {
            occlum_gdb_hook_load_elf(
                ldso_elf_base,
                ldso_path.as_ptr() as *const u8,
                ldso_path.len() as u64,
            );
        }
        let exec_elf_base = vm.get_elf_ranges()[0].start() as u64;
        unsafe {
            occlum_gdb_hook_load_elf(
                exec_elf_base,
                elf_path.as_ptr() as *const u8,
                elf_path.len() as u64,
            );
        }

        let task = {
            let ldso_entry = {
                let ldso_range = vm.get_elf_ranges()[1];
                let ldso_entry =
                    ldso_range.start() + ldso_elf_file.elf_header().entry_point() as usize;
                if !ldso_range.contains(ldso_entry) {
                    return_errno!(EINVAL, "Invalid program entry");
                }
                ldso_entry
            };
            let user_stack_base = vm.get_stack_base();
            let user_stack_limit = vm.get_stack_limit();
            let user_rsp = init_stack::do_init(user_stack_base, 4096, argv, envp, &auxtbl)?;
            unsafe {
                Task::new(
                    ldso_entry,
                    user_rsp,
                    user_stack_base,
                    user_stack_limit,
                    None,
                )?
            }
        };
        let vm_ref = Arc::new(SgxMutex::new(vm));
        let files_ref = {
            let files = init_files(parent_ref, file_actions)?;
            Arc::new(SgxMutex::new(files))
        };
        let rlimits_ref = Default::default();
        Process::new(&cwd, elf_path, task, vm_ref, files_ref, rlimits_ref)?
    };
    parent_adopts_new_child(&parent_ref, &new_process_ref);
    process_table::put(new_pid, new_process_ref.clone());
    let new_tid = new_pid;
    Ok((new_tid, new_process_ref))
}

#[derive(Debug)]
pub enum FileAction {
    /// open(path, oflag, mode) had been called, and the returned file
    /// descriptor, if not `fd`, had been changed to `fd`.
    Open {
        path: String,
        mode: u32,
        oflag: u32,
        fd: FileDesc,
    },
    Dup2(FileDesc, FileDesc),
    Close(FileDesc),
}

fn load_elf_to_vec(elf_path: &str, parent_ref: &ProcessRef) -> Result<Vec<u8>> {
    #[rustfmt::skip]
    parent_ref
        .lock()
        .unwrap()
        .lookup_inode(elf_path)
            .map_err(|e| errno!(e.errno(), "cannot find the ELF"))?
        .read_as_vec()
            .map_err(|e| errno!(e.errno(), "failed to read the executable ELF"))
}

fn init_files(parent_ref: &ProcessRef, file_actions: &[FileAction]) -> Result<FileTable> {
    // Usually, we just inherit the file table from the parent
    let parent = parent_ref.lock().unwrap();
    let should_inherit_file_table = parent.get_pid() > 0;
    if should_inherit_file_table {
        // Fork: clone file table
        let mut cloned_file_table = parent.get_files().lock().unwrap().clone();
        // Perform file actions to modify the cloned file table
        for file_action in file_actions {
            match file_action {
                &FileAction::Open {
                    ref path,
                    mode,
                    oflag,
                    fd,
                } => {
                    let file = parent.open_file(path.as_str(), oflag, mode)?;
                    let file_ref: Arc<Box<dyn File>> = Arc::new(file);
                    let creation_flags = CreationFlags::from_bits_truncate(oflag);
                    cloned_file_table.put_at(fd, file_ref, creation_flags.must_close_on_spawn());
                }
                &FileAction::Dup2(old_fd, new_fd) => {
                    let file = cloned_file_table.get(old_fd)?;
                    if old_fd != new_fd {
                        cloned_file_table.put_at(new_fd, file, false);
                    }
                }
                &FileAction::Close(fd) => {
                    // ignore error
                    cloned_file_table.del(fd);
                }
            }
        }
        // Exec: close fd with close_on_spawn
        cloned_file_table.close_on_spawn();
        return Ok(cloned_file_table);
    }
    drop(parent);

    // But, for init process, we initialize file table for it
    let mut file_table = FileTable::new();
    let stdin: Arc<Box<dyn File>> = Arc::new(Box::new(StdinFile::new()));
    let stdout: Arc<Box<dyn File>> = Arc::new(Box::new(StdoutFile::new()));
    // TODO: implement and use a real stderr
    let stderr = stdout.clone();
    file_table.put(stdin, false);
    file_table.put(stdout, false);
    file_table.put(stderr, false);
    Ok(file_table)
}

fn init_auxtbl(process_vm: &ProcessVM, exec_elf_file: &ElfFile) -> Result<AuxTable> {
    let mut auxtbl = AuxTable::new();
    auxtbl.set(AuxKey::AT_PAGESZ, 4096)?;
    auxtbl.set(AuxKey::AT_UID, 0)?;
    auxtbl.set(AuxKey::AT_GID, 0)?;
    auxtbl.set(AuxKey::AT_EUID, 0)?;
    auxtbl.set(AuxKey::AT_EGID, 0)?;
    auxtbl.set(AuxKey::AT_SECURE, 0)?;
    auxtbl.set(AuxKey::AT_SYSINFO, 0)?;

    let exec_elf_base = process_vm.get_elf_ranges()[0].start() as u64;
    let exec_elf_header = exec_elf_file.elf_header();
    auxtbl.set(AuxKey::AT_PHENT, exec_elf_header.ph_entry_size() as u64)?;
    auxtbl.set(AuxKey::AT_PHNUM, exec_elf_header.ph_count() as u64)?;
    auxtbl.set(AuxKey::AT_PHDR, exec_elf_base + exec_elf_header.ph_offset())?;
    auxtbl.set(
        AuxKey::AT_ENTRY,
        exec_elf_base + exec_elf_header.entry_point(),
    )?;

    let ldso_elf_base = process_vm.get_elf_ranges()[1].start() as u64;
    auxtbl.set(AuxKey::AT_BASE, ldso_elf_base)?;

    let syscall_addr = __occlum_syscall as *const () as u64;
    auxtbl.set(AuxKey::AT_OCCLUM_ENTRY, syscall_addr)?;
    // TODO: init AT_EXECFN
    // auxtbl.set_val(AuxKey::AT_EXECFN, "program_name")?;

    Ok(auxtbl)
}

fn parent_adopts_new_child(parent_ref: &ProcessRef, child_ref: &ProcessRef) {
    let mut parent = parent_ref.lock().unwrap();
    let mut child = child_ref.lock().unwrap();
    parent.children.push(Arc::downgrade(child_ref));
    child.parent = Some(parent_ref.clone());
}

extern "C" {
    fn __occlum_syscall(num: i32, arg0: u64, arg1: u64, arg2: u64, arg3: u64, arg4: u64) -> i64;
    fn occlum_gdb_hook_load_elf(elf_base: u64, elf_path: *const u8, elf_path_len: u64);
}
