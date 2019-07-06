use xmas_elf::symbol_table::Entry;
use xmas_elf::{header, program, sections, ElfFile};

use fs::{File, FileDesc, FileTable, INodeExt, OpenFlags, StdinFile, StdoutFile, ROOT_INODE};
use misc::ResourceLimitsRef;
use std::ffi::{CStr, CString};
use std::path::Path;
use std::sgxfs::SgxFile;
use vm::{ProcessVM};

use super::task::Task;
use super::*;

use self::init_stack::{AuxKey, AuxTable};

mod elf_helper;
mod init_stack;
mod init_vm;
mod segment;

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

pub fn do_spawn<P: AsRef<Path>>(
    elf_path: &P,
    argv: &[CString],
    envp: &[CString],
    file_actions: &[FileAction],
    parent_ref: &ProcessRef,
) -> Result<u32, Error> {
    let mut elf_buf = {
        let path = elf_path.as_ref().to_str().unwrap();
        let inode = parent_ref.lock().unwrap().lookup_inode(path)?;
        inode.read_as_vec()?
    };

    let elf_file = {
        let elf_file =
            ElfFile::new(&elf_buf).map_err(|e| (Errno::ENOEXEC, "Failed to parse the ELF file"))?;
        header::sanity_check(&elf_file)
            .map_err(|e| (Errno::ENOEXEC, "Failed to parse the ELF file"))?;
        /*
            elf_helper::print_program_headers(&elf_file)?;
            elf_helper::print_sections(&elf_file)?;
            elf_helper::print_pltrel_section(&elf_file)?;
        */
        elf_file
    };

    let (new_pid, new_process_ref) = {
        let cwd = parent_ref.lock().unwrap().get_cwd().to_owned();
        let vm = init_vm::do_init(&elf_file, &elf_buf[..])?;
        let base_addr = vm.get_base_addr();
        let program_entry = {
            let program_entry = base_addr + elf_helper::get_start_address(&elf_file)?;
            if !vm.get_code_range().contains(program_entry) {
                return errno!(EINVAL, "Invalid program entry");
            }
            program_entry
        };
        let auxtbl = init_auxtbl(base_addr, program_entry, &elf_file)?;
        let task = {
            let user_stack_base = vm.get_stack_base();
            let user_stack_limit = vm.get_stack_limit();
            let user_rsp = init_stack::do_init(user_stack_base, 4096, argv, envp, &auxtbl)?;
            unsafe {
                Task::new(program_entry, user_rsp,
                          user_stack_base, user_stack_limit, None)?
            }
        };
        let vm_ref = Arc::new(SgxMutex::new(vm));
        let files_ref = {
            let files = init_files(parent_ref, file_actions)?;
            Arc::new(SgxMutex::new(files))
        };
        let rlimits_ref = Default::default();
        Process::new(&cwd, task, vm_ref, files_ref, rlimits_ref)?
    };
    parent_adopts_new_child(&parent_ref, &new_process_ref);
    process_table::put(new_pid, new_process_ref.clone());
    task::enqueue_task(new_process_ref);
    Ok(new_pid)
}

fn init_files(parent_ref: &ProcessRef, file_actions: &[FileAction]) -> Result<FileTable, Error> {
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
                    let flags = OpenFlags::from_bits_truncate(oflag);
                    let file = parent.open_file(path.as_str(), flags, mode)?;
                    let file_ref: Arc<Box<File>> = Arc::new(file);

                    let close_on_spawn = flags.contains(OpenFlags::CLOEXEC);
                    cloned_file_table.put_at(fd, file_ref, close_on_spawn);
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
    let stdin: Arc<Box<File>> = Arc::new(Box::new(StdinFile::new()));
    let stdout: Arc<Box<File>> = Arc::new(Box::new(StdoutFile::new()));
    // TODO: implement and use a real stderr
    let stderr = stdout.clone();
    file_table.put(stdin, false);
    file_table.put(stdout, false);
    file_table.put(stderr, false);
    Ok(file_table)
}

fn init_auxtbl(
    base_addr: usize,
    program_entry: usize,
    elf_file: &ElfFile,
) -> Result<AuxTable, Error> {
    let mut auxtbl = AuxTable::new();
    auxtbl.set(AuxKey::AT_PAGESZ, 4096)?;
    auxtbl.set(AuxKey::AT_UID, 0)?;
    auxtbl.set(AuxKey::AT_GID, 0)?;
    auxtbl.set(AuxKey::AT_EUID, 0)?;
    auxtbl.set(AuxKey::AT_EGID, 0)?;
    auxtbl.set(AuxKey::AT_SECURE, 0)?;

    let ph = elf_helper::get_program_header_info(elf_file)?;
    auxtbl.set(AuxKey::AT_PHDR, (base_addr + ph.addr) as u64)?;
    auxtbl.set(AuxKey::AT_PHENT, ph.entry_size as u64)?;
    auxtbl.set(AuxKey::AT_PHNUM, ph.entry_num as u64)?;

    auxtbl.set(AuxKey::AT_ENTRY, program_entry as u64)?;

    auxtbl.set(AuxKey::AT_SYSINFO, 123)?;

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
}
