use self::init_stack::{AuxKey, AuxTable};
use super::task::Task;
use super::*;
use fs::{File, FileDesc, FileTable, StdinFile, StdoutFile /*, StderrFile*/};
use std::ffi::{CStr, CString};
use std::path::Path;
use std::sgxfs::SgxFile;
use vm::{ProcessVM, VMRangeTrait};
use xmas_elf::symbol_table::Entry;
use xmas_elf::{header, program, sections, ElfFile};

mod elf_helper;
mod init_stack;
mod init_vm;
mod segment;

#[derive(Debug)]
pub enum FileAction {
    // TODO: Add open action
    // Open(...)
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
        let key: sgx_key_128bit_t = [0 as uint8_t; 16];
        let mut sgx_file = SgxFile::open_ex(elf_path, &key)
            .map_err(|e| (Errno::ENOENT, "Failed to open the SGX-protected file"))?;

        let mut elf_buf = Vec::<u8>::new();
        sgx_file.read_to_end(&mut elf_buf);
        elf_buf
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
        let vm = init_vm::do_init(&elf_file, &elf_buf[..])?;
        let task = {
            let program_entry = {
                let program_entry = vm.get_base_addr() + elf_helper::get_start_address(&elf_file)?;
                if !vm.get_code_vma().contains_obj(program_entry, 16) {
                    return Err(Error::new(Errno::EINVAL, "Invalid program entry"));
                }
                program_entry
            };
            let stack_top = vm.get_stack_top();
            init_task(program_entry, stack_top, argv, envp)?
        };
        let files = init_files(parent_ref, file_actions)?;
        let exec_path = elf_path.as_ref().to_str().unwrap();
        Process::new(exec_path, task, vm, files)?
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
        let mut cloned_file_table = parent.get_files().clone();
        // Perform file actions to modify the cloned file table
        for file_action in file_actions {
            match file_action {
                FileAction::Dup2(old_fd, new_fd) => {
                    let file = cloned_file_table.get(*old_fd)?;
                    if old_fd != new_fd {
                        cloned_file_table.put_at(*new_fd, file, false);
                    }
                }
                FileAction::Close(fd) => {
                    cloned_file_table.del(*fd)?;
                }
            }
        }
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

fn init_task(
    user_entry: usize,
    stack_top: usize,
    argv: &[CString],
    envp: &[CString],
) -> Result<Task, Error> {
    let user_stack = init_stack(stack_top, argv, envp)?;
    Ok(Task {
        user_stack_addr: user_stack,
        user_entry_addr: user_entry,
        ..Default::default()
    })
}

fn init_stack(stack_top: usize, argv: &[CString], envp: &[CString]) -> Result<usize, Error> {
    let mut auxtbl = AuxTable::new();
    auxtbl.set_val(AuxKey::AT_PAGESZ, 4096)?;
    auxtbl.set_val(AuxKey::AT_UID, 0)?;
    auxtbl.set_val(AuxKey::AT_GID, 0)?;
    auxtbl.set_val(AuxKey::AT_EUID, 0)?;
    auxtbl.set_val(AuxKey::AT_EGID, 0)?;
    auxtbl.set_val(AuxKey::AT_SECURE, 0)?;

    init_stack::do_init(stack_top, 4096, argv, envp, &auxtbl)
}

fn parent_adopts_new_child(parent_ref: &ProcessRef, child_ref: &ProcessRef) {
    let mut parent = parent_ref.lock().unwrap();
    let mut child = child_ref.lock().unwrap();
    parent.children.push(Arc::downgrade(child_ref));
    child.parent = Some(parent_ref.clone());
}
