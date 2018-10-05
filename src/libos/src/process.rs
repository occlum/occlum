use prelude::*;
use {std, elf_helper, vma, syscall, file, file_table};
use std::{io, mem};
use std::path::Path;
use std::io::{Read};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sgxfs::SgxFile;
use std::thread;
use std::cell::Cell;

use xmas_elf::{ElfFile, header, program, sections};
use xmas_elf::symbol_table::Entry;

use vma::Vma;
use file::{File, StdinFile, StdoutFile/*, StderrFile*/};
use file_table::{FileTable};

lazy_static! {
    static ref PROCESS_TABLE: SgxMutex<HashMap<u32, ProcessRef>> = {
        SgxMutex::new(HashMap::new())
    };
}

fn put_into_pid_table(pid: u32, process: ProcessRef) {
    PROCESS_TABLE.lock().unwrap().insert(pid, process);
}

fn del_from_pid_table(pid: u32) {
    PROCESS_TABLE.lock().unwrap().remove(&pid);
}

fn look_up_pid_table(pid: u32) -> Option<ProcessRef> {
    PROCESS_TABLE.lock().unwrap().get(&pid).map(|pr| pr.clone())
}


static NEXT_PID : AtomicU32 = AtomicU32::new(1);

fn alloc_pid() -> u32 {
    NEXT_PID.fetch_add(1, Ordering::SeqCst)
}

fn free_pid(pid: u32) {
    // TODO:
}


pub fn do_spawn<P: AsRef<Path>>(elf_path: &P) -> Result<u32, Error> {
    let elf_buf = open_elf(elf_path)
        .map_err(|e| (e.errno, "Failed to open the ELF file"))?;

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

    let new_process = {
        let mut new_process = Process::new(&elf_file)
            .map_err(|e| (Errno::EUNDEF, "Failed to create the process"))?;

        {
            let file_table = &mut new_process.file_table;

            //let stdin = Arc::new(SgxMutex::new(Box::new(StdinFile::new())));
            let stdin : Arc<Box<File>> = Arc::new(Box::new(StdinFile::new()));
            let stdout : Arc<Box<File>> = Arc::new(Box::new(StdoutFile::new()));
            let stderr = stdout.clone();
            file_table.put(stdin);
            file_table.put(stdout);
            file_table.put(stderr);
        };

        new_process
    };

    let new_pid = new_process.pid;
    let new_process_ref = Arc::new(SgxMutex::new(new_process));
    enqueue_new_process(new_process_ref.clone());
    put_into_pid_table(new_pid, new_process_ref.clone());

    // FIXME: if ocall_new_task failed, then new_process will not be dropped
    let mut ret = 0;
    let ocall_status = unsafe { ocall_run_new_task(&mut ret) };
    if ocall_status != sgx_status_t::SGX_SUCCESS || ret != 0 {
        return Err((Errno::EUNDEF, "Failed to start the process").into());
    }

    Ok(new_pid)
}

thread_local! {
    static _CURRENT_PROCESS_PTR: Cell<*const SgxMutex<Process>> =
        Cell::new(0 as *const SgxMutex<Process>);
}

pub fn set_current(process: &ProcessRef) {
    let process_ref_clone = process.clone();
    let process_ptr = Arc::into_raw(process_ref_clone);

    _CURRENT_PROCESS_PTR.with(|cp| {
        cp.set(process_ptr);
    });
}

pub fn reset_current() {
    let mut process_ptr = 0 as *const SgxMutex<Process>;
    _CURRENT_PROCESS_PTR.with(|cp| {
        process_ptr = cp.get();
        cp.set(0 as *const SgxMutex<Process>);
    });

    // Prevent memory leakage
    unsafe { drop(Arc::from_raw(process_ptr)); }
}

pub fn get_current() -> &'static SgxMutex<Process> {
    let mut process_ptr : *const SgxMutex<Process> = 0 as *const SgxMutex<Process>;
    _CURRENT_PROCESS_PTR.with(|cp| {
        process_ptr = cp.get();
    });
    unsafe {
        mem::transmute::<*const SgxMutex<Process>, &'static SgxMutex<Process>>(process_ptr)
    }
}

pub fn do_getpid() -> u32 {
    let current_ref = get_current();
    let current_process = current_ref.lock().unwrap();
    current_process.pid
}

pub fn do_exit(exit_code: i32) {
    {
        let current_ref = get_current();
        let mut current_process = current_ref.lock().unwrap();
        current_process.exit_code = exit_code;
        current_process.status = Status::ZOMBIE;
    }
}

pub fn do_wait4(child_pid: u32) -> Result<i32, Error> {
    let child_process = look_up_pid_table(child_pid)
        .ok_or_else(|| (Errno::ECHILD, "Cannot find child process with the given PID"))?;
    let mut exit_code = 0;
    loop {
        let guard = child_process.lock().unwrap();
        if guard.status == Status::ZOMBIE {
            exit_code = guard.exit_code;
            break;
        }
        drop(guard);
    }

    let child_pid = child_process.lock().unwrap().pid;
    del_from_pid_table(child_pid);

    Ok(exit_code)
}

pub fn run_task() -> Result<(), Error> {
    let new_process : ProcessRef = dequeue_new_process()
        .ok_or_else(|| (Errno::EAGAIN, "No new processes to run"))?;
    set_current(&new_process);

    let pid;
    let new_task;
    {
        let guard = new_process.lock().unwrap();
        let process : &Process = &guard;
        pid = process.pid;
        //println!("Run process: {:#x?}", process);
        //println!("Run process (pid = {})", process.pid);
        new_task = &process.task as *const Task
    };

    unsafe { do_run_task(new_task as *const Task); }

    // Init process does not have any parent, so it has to release itself
    if pid == 1 {
        del_from_pid_table(1);
    }

    reset_current();

    Ok(())
}

fn open_elf<P: AsRef<Path>>(path: &P) -> Result<Vec<u8>, Error> {
    let key : sgx_key_128bit_t = [0 as uint8_t; 16];
    let mut elf_file = SgxFile::open_ex(path, &key)
        .map_err(|e| (Errno::ENOENT, "Failed to open the SGX-protected file"))?;

    let mut elf_buf = Vec::<u8>::new();
    elf_file.read_to_end(&mut elf_buf);

    Ok(elf_buf)
}


#[derive(Clone, Debug, Default)]
#[repr(C)]
pub struct Process {
    pub task: Task,
    pub status: Status,
    pub pid: u32,
    pub exit_code: i32,
    pub code_vma: Vma,
    pub data_vma: Vma,
    pub stack_vma: Vma,
    pub program_base_addr: usize,
    pub program_entry_addr: usize,
    pub file_table: FileTable,
}
pub type ProcessRef = Arc<SgxMutex<Process>>;

impl Process {
    pub fn new(elf_file: &ElfFile) -> Result<Process, Error> {
        let mut new_process : Process = Default::default();
        new_process.create_process_image(elf_file)?;
        new_process.link_syscalls(elf_file)?;
        new_process.mprotect()?;

        new_process.task = Task {
            user_stack_addr: new_process.stack_vma.mem_end - 16,
            user_entry_addr: new_process.program_entry_addr,
            fs_base_addr: 0,
            .. Default::default()
        };

        new_process.pid = alloc_pid();

        Ok(new_process)
    }

    fn create_process_image(self: &mut Process, elf_file: &ElfFile)
        -> Result<(), Error>
    {
        let code_ph = elf_helper::get_code_program_header(elf_file)
            .map_err(|e| (Errno::ENOEXEC, "Failed to get the program header of code"))?;
        let data_ph = elf_helper::get_data_program_header(elf_file)
            .map_err(|e| (Errno::ENOEXEC, "Failed to get the program header of code"))?;

        self.code_vma = Vma::from_program_header(&code_ph)?;
        self.data_vma = Vma::from_program_header(&data_ph)?;
        self.stack_vma = Vma::new(32 * 1024 * 1024, 4096,
            vma::Perms(vma::PERM_R | vma::PERM_W))?;

        self.program_base_addr = self.alloc_mem_for_vmas(elf_file)?;
        self.program_entry_addr = self.program_base_addr +
            elf_helper::get_start_address(elf_file)?;
        if !self.code_vma.contains(self.program_entry_addr) {
            return Err((Errno::EINVAL, "Entry address is out of the code segment").into());
        }

        Ok(())
    }

    fn alloc_mem_for_vmas(self: &mut Process, elf_file: &ElfFile)
        -> Result<usize, Error>
    {
        let mut vma_list = vec![&mut self.code_vma, &mut self.data_vma, &mut self.stack_vma];
        let base_addr = vma::malloc_batch(&mut vma_list, elf_file.input)?;

        Ok(base_addr)
    }

    fn link_syscalls(self: &mut Process, elf_file: &ElfFile)
        -> Result<(), Error>
    {
        let syscall_addr = rusgx_syscall as *const () as usize;

        let rela_entries = elf_helper::get_pltrel_entries(&elf_file)?;
        let dynsym_entries = elf_helper::get_dynsym_entries(&elf_file)?;
        for rela_entry in rela_entries {
            let dynsym_idx = rela_entry.get_symbol_table_index() as usize;
            let dynsym_entry = &dynsym_entries[dynsym_idx];
            let dynsym_str = dynsym_entry.get_name(elf_file)
                .map_err(|e| Error::new(Errno::ENOEXEC,
                                        "Failed to get the name of dynamic symbol"))?;

            if dynsym_str == "rusgx_syscall" {
                let rela_addr = self.program_base_addr + rela_entry.get_offset() as usize;
                unsafe {
                    std::ptr::write_unaligned(rela_addr as *mut usize, syscall_addr);
                }
            }
        }

        Ok(())
    }

    fn mprotect(self: &mut Process) -> Result<(), Error> {
        let vma_list = vec![&self.code_vma, &self.data_vma, &self.stack_vma];
        vma::mprotect_batch(&vma_list)
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        free_pid(self.pid);
    }
}

/// Note: this definition must be in sync with task.h
#[derive(Clone, Debug, Default)]
#[repr(C)]
pub struct Task {
    pub syscall_stack_addr: usize,
    pub user_stack_addr: usize,
    pub user_entry_addr: usize,
    pub fs_base_addr: usize,
    pub saved_state: usize, // struct jmpbuf*
}

lazy_static! {
    static ref new_process_queue: SgxMutex<VecDeque<ProcessRef>> = {
        SgxMutex::new(VecDeque::new())
    };
}


#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Status {
    RUNNING,
    INTERRUPTIBLE,
    ZOMBIE,
    STOPPED,
}

impl Default for Status {
    fn default() -> Status {
        Status::RUNNING
    }
}


fn dequeue_new_process() -> Option<ProcessRef> {
    new_process_queue.lock().unwrap().pop_front()
}

fn enqueue_new_process(new_process: ProcessRef) {
    new_process_queue.lock().unwrap().push_back(new_process)
}


extern {
    fn ocall_run_new_task(ret: *mut i32) -> sgx_status_t;
    fn do_run_task(task: *const Task) -> i32;
    fn do_exit_task();
    fn rusgx_syscall(num: i32, arg0: u64, arg1: u64, arg2: u64, arg3: u64, arg4: u64) -> i64;
}
