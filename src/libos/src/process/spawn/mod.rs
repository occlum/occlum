use super::*;
use fs::{File, StdinFile, StdoutFile/*, StderrFile*/, FileTable};
use std::path::Path;
use std::ffi::{CStr, CString};
use std::sgxfs::SgxFile;
use xmas_elf::{ElfFile, header, program, sections};
use xmas_elf::symbol_table::Entry;
use self::init_stack::{AuxKey, AuxTable};
use super::task::{Task};
use vm::{ProcessVM, VMRangeTrait};

mod init_stack;
mod init_vm;
mod elf_helper;
mod segment;

pub fn do_spawn<P: AsRef<Path>>(elf_path: &P, argv: &[CString], envp: &[CString])
    -> Result<u32, Error>
{
    let mut elf_buf = {
        let key : sgx_key_128bit_t = [0 as uint8_t; 16];
        let mut sgx_file = SgxFile::open_ex(elf_path, &key)
            .map_err(|e| (Errno::ENOENT, "Failed to open the SGX-protected file"))?;

        let mut elf_buf = Vec::<u8>::new();
        sgx_file.read_to_end(&mut elf_buf);
        elf_buf
    };

    let elf_file = {
        let elf_file = ElfFile::new(&elf_buf)
            .map_err(|e| (Errno::ENOEXEC, "Failed to parse the ELF file"))?;
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
                let program_entry = vm.get_base_addr() +
                                    elf_helper::get_start_address(&elf_file)?;
                if !vm.get_code_vma().contains_obj(program_entry, 16) {
                    return Err(Error::new(Errno::EINVAL, "Invalid program entry"));
                }
                program_entry
            };
            let stack_top = vm.get_stack_top();
            init_task(program_entry, stack_top, argv, envp)?
        };
        let files = init_files()?;
        let exec_path = elf_path.as_ref().to_str().unwrap();
        Process::new(exec_path, task, vm, files)?
    };
    process_table::put(new_pid, new_process_ref.clone());
    task::enqueue_task(new_process_ref);
    Ok(new_pid)
}

fn init_files() -> Result<FileTable, Error> {
    let mut file_table = FileTable::new();

    let stdin : Arc<Box<File>> = Arc::new(Box::new(StdinFile::new()));
    let stdout : Arc<Box<File>> = Arc::new(Box::new(StdoutFile::new()));
    // TODO: implement and use a real stderr
    let stderr = stdout.clone();
    file_table.put(stdin);
    file_table.put(stdout);
    file_table.put(stderr);

    Ok(file_table)
}

fn init_task(user_entry: usize, stack_top: usize,
             argv: &[CString], envp: &[CString])
    -> Result<Task, Error>
{
    let user_stack = init_stack(stack_top, argv, envp)?;
    Ok(Task {
        user_stack_addr: user_stack,
        user_entry_addr: user_entry,
        fs_base_addr: 0,
        .. Default::default()
    })
}

fn init_stack(stack_top: usize, argv: &[CString], envp: &[CString])
    -> Result<usize, Error>
{
    let mut auxtbl = AuxTable::new();
    auxtbl.set_val(AuxKey::AT_PAGESZ, 4096)?;
    auxtbl.set_val(AuxKey::AT_UID, 0)?;
    auxtbl.set_val(AuxKey::AT_GID, 0)?;
    auxtbl.set_val(AuxKey::AT_EUID, 0)?;
    auxtbl.set_val(AuxKey::AT_EGID, 0)?;
    auxtbl.set_val(AuxKey::AT_SECURE, 0)?;

    init_stack::do_init(stack_top, 4096, argv, envp, &auxtbl)
}
