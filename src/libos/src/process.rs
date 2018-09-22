use std;
use std::vec::Vec;
use std::path::Path;
use std::sgxfs::SgxFile;
use std::io;
use std::io::{Read};

use sgx_types::*;

use xmas_elf::{ElfFile, header, program};
use xmas_elf::sections;
use xmas_elf::symbol_table::Entry;

use {elf_helper, vma, syscall};
use vma::Vma;
use std::sync::atomic::AtomicU32;
use std::sync::SgxMutex;
use std::sync::Arc;
use std::collections::{HashMap, VecDeque};

//static next_pid : AtomicU32 = AtomicU32::new(42);

lazy_static! {
    static ref process_table: SgxMutex<HashMap<u32, Arc<SgxMutex<Process>>>> = {
        SgxMutex::new(HashMap::new())
    };
}

pub fn spawn_process<P: AsRef<Path>>(elf_path: &P) -> Result<(), &'static str> {
    let elf_buf = open_elf(elf_path).unwrap();
    let elf_file = ElfFile::new(&elf_buf).unwrap();
    header::sanity_check(&elf_file).unwrap();
/*
    elf_helper::print_program_headers(&elf_file)?;
    elf_helper::print_sections(&elf_file)?;
    elf_helper::print_pltrel_section(&elf_file)?;
*/
    let new_process = Process::new(&elf_file)?;
    //println!("new_process: {:#x?}", &new_process);
    let new_task = Task::from(&new_process);

    process_table.lock().unwrap()
        .insert(0, Arc::new(SgxMutex::new(new_process)));
    new_task_queue.lock().unwrap()
        .push_back(new_task);

    let mut ret = 0;
    let ocall_status = unsafe { ocall_run_new_task(&mut ret) };
    if ocall_status != sgx_status_t::SGX_SUCCESS || ret != 0 {
        return Err("ocall_run_new_task failed");
    }

    Ok(())
}


pub fn run_task() -> Result<(), &'static str> {
    if let Some(new_task) = pop_new_task() {
        println!("Run task: {:#x?}", &new_task);
        println!("do_run_task() begin: {}", do_run_task as *const () as usize);
        unsafe { do_run_task(&new_task as *const Task); }
        println!("do_run_task() end");
    }
    Ok(())
}

fn open_elf<P: AsRef<Path>>(path: &P) -> io::Result<Vec<u8>> {
    let key : sgx_key_128bit_t = [0 as uint8_t; 16];
    let mut elf_file = SgxFile::open_ex(path, &key)?;

    let mut elf_buf = Vec::<u8>::new();
    elf_file.read_to_end(&mut elf_buf);
    Ok(elf_buf)
}


#[derive(Clone, Debug, Default)]
#[repr(C)]
pub struct Process {
    pub code_vma: Vma,
    pub data_vma: Vma,
    pub stack_vma: Vma,
    pub program_base_addr: usize,
    pub program_entry_addr: usize,
}

impl Process {
    pub fn new(elf_file: &ElfFile) -> Result<Process, &'static str> {
        let mut new_process : Process = Default::default();
        new_process.create_process_image(elf_file)?;
        new_process.link_syscalls(elf_file)?;
        new_process.mprotect()?;
        Ok(new_process)
    }

    fn create_process_image(self: &mut Process, elf_file: &ElfFile)
        -> Result<(), &'static str>
    {
        let code_ph = elf_helper::get_code_program_header(elf_file)?;
        let data_ph = elf_helper::get_data_program_header(elf_file)?;

        self.code_vma = Vma::from_program_header(&code_ph)?;
        self.data_vma = Vma::from_program_header(&data_ph)?;
        self.stack_vma = Vma::new(8 * 1024, 4096,
            vma::Perms(vma::PERM_R | vma::PERM_W))?;

        self.program_base_addr = self.alloc_mem_for_vmas(elf_file)?;
        self.program_entry_addr = self.program_base_addr +
            elf_helper::get_start_address(elf_file)?;
        if !self.code_vma.contains(self.program_entry_addr) {
            return Err("Entry address is out of the code segment");
        }

        Ok(())
    }

    fn alloc_mem_for_vmas(self: &mut Process, elf_file: &ElfFile)
        -> Result<usize, &'static str>
    {
        let mut vma_list = vec![&mut self.code_vma, &mut self.data_vma, &mut self.stack_vma];
        let base_addr = vma::malloc_batch(&mut vma_list, elf_file.input)?;

        Ok(base_addr)
    }

    fn link_syscalls(self: &mut Process, elf_file: &ElfFile)
        -> Result<(), &'static str>
    {
        let syscall_addr = rusgx_syscall as *const () as usize;

        let rela_entries = elf_helper::get_pltrel_entries(&elf_file)?;
        let dynsym_entries = elf_helper::get_dynsym_entries(&elf_file)?;
        for rela_entry in rela_entries {
            let dynsym_idx = rela_entry.get_symbol_table_index() as usize;
            let dynsym_entry = &dynsym_entries[dynsym_idx];
            let dynsym_str = dynsym_entry.get_name(&elf_file)?;

            if dynsym_str == "rusgx_syscall" {
                let rela_addr = self.program_base_addr + rela_entry.get_offset() as usize;
                unsafe {
                    std::ptr::write_unaligned(rela_addr as *mut usize, syscall_addr);
                }
            }
        }

        Ok(())
    }

    fn mprotect(self: &mut Process) -> Result<(), &'static str> {
        let vma_list = vec![&self.code_vma, &self.data_vma, &self.stack_vma];
        vma::mprotect_batch(&vma_list)
    }
}


#[derive(Clone, Debug, Default)]
#[repr(C)]
pub struct Task {
    pub pid: u32,
    pub exit_code: u32,
    pub syscall_stack_addr: usize,
    pub user_stack_addr: usize,
    pub user_entry_addr: usize,
    pub fs_base_addr: usize,
    pub saved_state: usize, // struct jmpbuf*
}

impl<'a> From<&'a Process> for Task {
    fn from(process: &'a Process) -> Task {
        Task {
            pid: 1234,
            user_stack_addr: process.stack_vma.mem_end - 16,
            user_entry_addr: process.program_entry_addr,
            fs_base_addr: 0,
            .. Default::default()
        }
    }
}

lazy_static! {
    static ref new_task_queue: Arc<SgxMutex<VecDeque<Task>>> = {
        Arc::new(SgxMutex::new(VecDeque::new()))
    };
}

fn pop_new_task() -> Option<Task> {
    new_task_queue.lock().unwrap().pop_front()
}


extern {
    fn ocall_run_new_task(ret: *mut i32) -> sgx_status_t;
    fn do_run_task(task: *const Task) -> i32;
    fn rusgx_syscall(num: i32, arg0: u64, arg1: u64, arg2: u64, arg3: u64, arg4: u64) -> i64;
}
