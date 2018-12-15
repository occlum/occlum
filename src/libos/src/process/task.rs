use super::*;
use std::{mem};

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

pub fn enqueue_task(new_process: ProcessRef) {
    new_process_queue.lock().unwrap().push_back(new_process);

    let mut ret = 0;
    let ocall_status = unsafe { ocall_run_new_task(&mut ret) };
    if ocall_status != sgx_status_t::SGX_SUCCESS || ret != 0 {
        panic!("Failed to start the process");
    }
}

fn dequeue_task() -> Option<ProcessRef> {
    new_process_queue.lock().unwrap().pop_front()
}


pub fn run_task() -> Result<(), Error> {
    let new_process : ProcessRef = dequeue_task()
        .ok_or_else(|| (Errno::EAGAIN, "No new processes to run"))?;
    set_current(&new_process);

    let (pid, task) = {
        let mut process = new_process.lock().unwrap();
        let pid = process.get_pid();
        let task = process.get_task_mut() as *mut Task;
        (pid, task)
    };

    unsafe {
        // task may only be modified by this function; so no lock is needed
        do_run_task(task);
    }

    // Init process does not have any parent, so it has to release itself
    if pid == 1 {
        process_table::remove(1);
    }

    reset_current();
    Ok(())
}


thread_local! {
    static _CURRENT_PROCESS_PTR: Cell<*const SgxMutex<Process>> =
        Cell::new(0 as *const SgxMutex<Process>);
}

pub fn get_current() -> &'static SgxMutex<Process> {
    let mut process_ptr = 0 as *const SgxMutex<Process>;
    _CURRENT_PROCESS_PTR.with(|cp| {
        process_ptr = cp.get();
    });
    unsafe { mem::transmute(process_ptr) }
}

fn set_current(process: &ProcessRef) {
    let process_ref_clone = process.clone();
    let process_ptr = Arc::into_raw(process_ref_clone);

    _CURRENT_PROCESS_PTR.with(|cp| {
        cp.set(process_ptr);
    });
}

fn reset_current() {
    let mut process_ptr = 0 as *const SgxMutex<Process>;
    _CURRENT_PROCESS_PTR.with(|cp| {
        process_ptr = cp.get();
        cp.set(0 as *const SgxMutex<Process>);
    });

    // Prevent memory leakage
    unsafe { drop(Arc::from_raw(process_ptr)); }
}

extern {
    fn ocall_run_new_task(ret: *mut i32) -> sgx_status_t;
    fn do_run_task(task: *mut Task) -> i32;
}
