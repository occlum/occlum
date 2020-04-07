use std::ffi::{CStr, CString};
use std::path::Path;

use self::aux_vec::{AuxKey, AuxVec};
use super::elf_file::{ElfFile, ElfHeader, ProgramHeader, ProgramHeaderExt};
use super::process::ProcessBuilder;
use super::task::Task;
use super::{table, task, ProcessRef, ThreadRef};
use crate::fs::{
    CreationFlags, File, FileDesc, FileMode, FileTable, FsView, HostStdioFds, INodeExt, StdinFile,
    StdoutFile, ROOT_INODE,
};
use crate::prelude::*;
use crate::vm::ProcessVM;

mod aux_vec;
mod init_stack;
mod init_vm;

/// Spawn a new process and execute it in a new host thread.
pub fn do_spawn(
    elf_path: &str,
    argv: &[CString],
    envp: &[CString],
    file_actions: &[FileAction],
    current_ref: &ThreadRef,
) -> Result<pid_t> {
    let exec_now = true;
    do_spawn_common(
        elf_path,
        argv,
        envp,
        file_actions,
        None,
        current_ref,
        exec_now,
    )
}

/// Spawn a new process but execute it later.
pub fn do_spawn_without_exec(
    elf_path: &str,
    argv: &[CString],
    envp: &[CString],
    file_actions: &[FileAction],
    host_stdio_fds: &HostStdioFds,
    current_ref: &ThreadRef,
) -> Result<pid_t> {
    let exec_now = false;
    do_spawn_common(
        elf_path,
        argv,
        envp,
        file_actions,
        Some(host_stdio_fds),
        current_ref,
        exec_now,
    )
}

fn do_spawn_common(
    elf_path: &str,
    argv: &[CString],
    envp: &[CString],
    file_actions: &[FileAction],
    host_stdio_fds: Option<&HostStdioFds>,
    current_ref: &ThreadRef,
    exec_now: bool,
) -> Result<pid_t> {
    let new_process_ref = new_process(
        elf_path,
        argv,
        envp,
        file_actions,
        host_stdio_fds,
        current_ref,
    )?;

    let new_main_thread = new_process_ref
        .main_thread()
        .expect("the main thread is just created; it must exist");
    if exec_now {
        task::enqueue_and_exec(new_main_thread);
    } else {
        task::enqueue(new_main_thread);
    };

    let new_pid = new_process_ref.pid();
    Ok(new_pid)
}

/// Create a new process and its main thread.
fn new_process(
    elf_path: &str,
    argv: &[CString],
    envp: &[CString],
    file_actions: &[FileAction],
    host_stdio_fds: Option<&HostStdioFds>,
    current_ref: &ThreadRef,
) -> Result<ProcessRef> {
    let elf_buf = load_elf_to_vec(elf_path, current_ref)
        .cause_err(|e| errno!(e.errno(), "cannot load the executable"))?;
    let ldso_path = "/lib/ld-musl-x86_64.so.1";
    let ldso_elf_buf = load_elf_to_vec(ldso_path, current_ref)
        .cause_err(|e| errno!(e.errno(), "cannot load ld.so"))?;

    let exec_elf_file =
        ElfFile::new(&elf_buf).cause_err(|e| errno!(e.errno(), "invalid executable"))?;
    let ldso_elf_file =
        ElfFile::new(&ldso_elf_buf).cause_err(|e| errno!(e.errno(), "invalid ld.so"))?;

    let new_process_ref = {
        let process_ref = current_ref.process().clone();

        let vm = init_vm::do_init(&exec_elf_file, &ldso_elf_file)?;
        let auxvec = init_auxvec(&vm, &exec_elf_file)?;

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
            let user_rsp = init_stack::do_init(user_stack_base, 4096, argv, envp, &auxvec)?;
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
            let files = init_files(current_ref, file_actions, host_stdio_fds)?;
            Arc::new(SgxMutex::new(files))
        };
        let fs_ref = Arc::new(SgxMutex::new(current_ref.fs().lock().unwrap().clone()));

        ProcessBuilder::new()
            .vm(vm_ref)
            .exec_path(elf_path)
            .parent(process_ref)
            .task(task)
            .fs(fs_ref)
            .files(files_ref)
            .build()?
    };

    table::add_process(new_process_ref.clone());
    table::add_thread(new_process_ref.main_thread().unwrap());

    info!(
        "Process created: elf = {}, pid = {}",
        elf_path,
        new_process_ref.pid()
    );

    Ok(new_process_ref)
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

fn load_elf_to_vec(elf_path: &str, current_ref: &ThreadRef) -> Result<Vec<u8>> {
    let inode = current_ref
        .fs()
        .lock()
        .unwrap()
        .lookup_inode(elf_path)
        .map_err(|e| errno!(e.errno(), "cannot find the ELF"))?;
    let file_mode = {
        let info = inode.metadata()?;
        FileMode::from_bits_truncate(info.mode)
    };
    if !file_mode.is_executable() {
        return_errno!(EACCES, "elf file is not executable");
    }
    if file_mode.has_set_uid() || file_mode.has_set_gid() {
        warn!(
            "set-user-ID and set-group-ID are not supportted, FileMode:{:?}",
            file_mode
        );
    }
    inode
        .read_as_vec()
        .map_err(|e| errno!(e.errno(), "failed to read the executable ELF"))
}

fn init_files(
    current_ref: &ThreadRef,
    file_actions: &[FileAction],
    host_stdio_fds: Option<&HostStdioFds>,
) -> Result<FileTable> {
    // Usually, we just inherit the file table from the current process
    let should_inherit_file_table = current_ref.process().pid() > 0;
    if should_inherit_file_table {
        // Fork: clone file table
        let mut cloned_file_table = current_ref.files().lock().unwrap().clone();
        // Perform file actions to modify the cloned file table
        for file_action in file_actions {
            match file_action {
                &FileAction::Open {
                    ref path,
                    mode,
                    oflag,
                    fd,
                } => {
                    let file =
                        current_ref
                            .fs()
                            .lock()
                            .unwrap()
                            .open_file(path.as_str(), oflag, mode)?;
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

    // But, for init process, we initialize file table for it
    let mut file_table = FileTable::new();
    let stdin: Arc<Box<dyn File>> = Arc::new(Box::new(StdinFile::new(
        host_stdio_fds.unwrap().stdin_fd as FileDesc,
    )));
    let stdout: Arc<Box<dyn File>> = Arc::new(Box::new(StdoutFile::new(
        host_stdio_fds.unwrap().stdout_fd as FileDesc,
    )));
    let stderr: Arc<Box<dyn File>> = Arc::new(Box::new(StdoutFile::new(
        host_stdio_fds.unwrap().stderr_fd as FileDesc,
    )));

    file_table.put(stdin, false);
    file_table.put(stdout, false);
    file_table.put(stderr, false);
    Ok(file_table)
}

fn init_auxvec(process_vm: &ProcessVM, exec_elf_file: &ElfFile) -> Result<AuxVec> {
    let mut auxvec = AuxVec::new();
    auxvec.set(AuxKey::AT_PAGESZ, 4096)?;
    auxvec.set(AuxKey::AT_UID, 0)?;
    auxvec.set(AuxKey::AT_GID, 0)?;
    auxvec.set(AuxKey::AT_EUID, 0)?;
    auxvec.set(AuxKey::AT_EGID, 0)?;
    auxvec.set(AuxKey::AT_SECURE, 0)?;
    auxvec.set(AuxKey::AT_SYSINFO, 0)?;

    let exec_elf_base = process_vm.get_elf_ranges()[0].start() as u64;
    let exec_elf_header = exec_elf_file.elf_header();
    auxvec.set(AuxKey::AT_PHENT, exec_elf_header.ph_entry_size() as u64)?;
    auxvec.set(AuxKey::AT_PHNUM, exec_elf_header.ph_count() as u64)?;
    auxvec.set(AuxKey::AT_PHDR, exec_elf_base + exec_elf_header.ph_offset())?;
    auxvec.set(
        AuxKey::AT_ENTRY,
        exec_elf_base + exec_elf_header.entry_point(),
    )?;

    let ldso_elf_base = process_vm.get_elf_ranges()[1].start() as u64;
    auxvec.set(AuxKey::AT_BASE, ldso_elf_base)?;

    let syscall_addr = __occlum_syscall as *const () as u64;
    auxvec.set(AuxKey::AT_OCCLUM_ENTRY, syscall_addr)?;
    // TODO: init AT_EXECFN
    // auxvec.set_val(AuxKey::AT_EXECFN, "program_name")?;

    Ok(auxvec)
}

extern "C" {
    fn __occlum_syscall(num: i32, arg0: u64, arg1: u64, arg2: u64, arg3: u64, arg4: u64) -> i64;
    fn occlum_gdb_hook_load_elf(elf_base: u64, elf_path: *const u8, elf_path_len: u64);
}
