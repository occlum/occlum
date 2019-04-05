use xmas_elf::{ElfFile, header, program, sections};
use xmas_elf::symbol_table::Entry;

use fs::{File, FileDesc, FileTable, INodeExt, ROOT_INODE, StdinFile, StdoutFile};
use std::ffi::{CStr, CString};
use std::path::Path;
use std::sgxfs::SgxFile;
use vm::{ProcessVM, VMRangeTrait};

use super::*;
use super::task::Task;

use self::init_stack::{AuxKey, AuxTable};

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
        let path = elf_path.as_ref().to_str().unwrap().trim_start_matches('/');
        let inode = ROOT_INODE.lookup(path)?;
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
            if !vm.get_code_vma().contains_obj(program_entry, 16) {
                return Err(Error::new(Errno::EINVAL, "Invalid program entry"));
            }
            program_entry
        };
        let auxtbl = init_auxtbl(base_addr, program_entry, &elf_file)?;
        let task = {
            let stack_top = vm.get_stack_top();
            init_task(program_entry, stack_top, argv, envp, &auxtbl)?
        };
        let vm_ref = Arc::new(SgxMutex::new(vm));
        let files_ref = {
            let files = init_files(parent_ref, file_actions)?;
            Arc::new(SgxMutex::new(files))
        };
        Process::new(&cwd, task, vm_ref, files_ref)?
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
        let mut cloned_file_table = parent.get_files().lock().unwrap().clone();
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
    auxtbl: &AuxTable,
) -> Result<Task, Error> {
    let user_stack = init_stack::do_init(stack_top, 4096, argv, envp, auxtbl)?;
    Ok(Task {
        user_stack_addr: user_stack,
        user_entry_addr: user_entry,
        ..Default::default()
    })
}

fn init_auxtbl(base_addr: usize, program_entry: usize, elf_file: &ElfFile) -> Result<AuxTable, Error> {
    let mut auxtbl = AuxTable::new();
    auxtbl.set_val(AuxKey::AT_PAGESZ, 4096)?;
    auxtbl.set_val(AuxKey::AT_UID, 0)?;
    auxtbl.set_val(AuxKey::AT_GID, 0)?;
    auxtbl.set_val(AuxKey::AT_EUID, 0)?;
    auxtbl.set_val(AuxKey::AT_EGID, 0)?;
    auxtbl.set_val(AuxKey::AT_SECURE, 0)?;

    let ph = elf_helper::get_program_header_info(elf_file)?;
    auxtbl.set_val(AuxKey::AT_PHDR, (base_addr + ph.addr) as u64)?;
    auxtbl.set_val(AuxKey::AT_PHENT, ph.entry_size as u64)?;
    auxtbl.set_val(AuxKey::AT_PHNUM, ph.entry_num as u64)?;

    auxtbl.set_val(AuxKey::AT_ENTRY, program_entry as u64)?;
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
