use std::mem;

use super::*;

/// Note: this definition must be in sync with task.h
#[derive(Clone, Debug, Default)]
#[repr(C)]
pub struct Task {
    pub kernel_stack_addr: usize,
    pub kernel_fsbase_addr: usize,
    pub user_stack_addr: usize,
    pub user_fsbase_addr: usize,
    pub user_entry_addr: usize,
    pub saved_state: usize, // struct jmpbuf*
}

lazy_static! {
    static ref NEW_PROCESS_QUEUE: SgxMutex<VecDeque<ProcessRef>> =
        { SgxMutex::new(VecDeque::new()) };
}

pub fn enqueue_task(new_process: ProcessRef) {
    NEW_PROCESS_QUEUE.lock().unwrap().push_back(new_process);

    let mut ret = 0;
    let ocall_status = unsafe { ocall_run_new_task(&mut ret) };
    if ocall_status != sgx_status_t::SGX_SUCCESS || ret != 0 {
        panic!("Failed to start the process");
    }
}

fn dequeue_task() -> Option<ProcessRef> {
    NEW_PROCESS_QUEUE.lock().unwrap().pop_front()
}

pub fn run_task() -> Result<i32, Error> {
    let new_process: ProcessRef =
        dequeue_task().ok_or_else(|| (Errno::EAGAIN, "No new processes to run"))?;
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

    let exit_status = {
        let mut process = new_process.lock().unwrap();
        process.get_exit_status()
    };

    // Init process does not have any parent, so it has to release itself
    if pid == 1 {
        process_table::remove(1);
    }

    reset_current();
    Ok(exit_status)
}

thread_local! {
    static _CURRENT_PROCESS_PTR: Cell<*const SgxMutex<Process>> = {
        Cell::new(0 as *const SgxMutex<Process>)
    };
}

pub fn get_current() -> ProcessRef {
    let current_ptr = _CURRENT_PROCESS_PTR.with(|cell| cell.get());

    let current_ref = unsafe { Arc::from_raw(current_ptr) };
    let current_ref_clone = current_ref.clone();
    Arc::into_raw(current_ref);

    current_ref_clone
}

fn set_current(process: &ProcessRef) {
    let process_ref_clone = process.clone();
    let process_ptr = Arc::into_raw(process_ref_clone);

    _CURRENT_PROCESS_PTR.with(|cp| {
        cp.set(process_ptr);
    });
}

fn reset_current() {
    let mut process_ptr = _CURRENT_PROCESS_PTR.with(|cp| cp.replace(0 as *const SgxMutex<Process>));

    // Prevent memory leakage
    unsafe {
        drop(Arc::from_raw(process_ptr));
    }
}

extern "C" {
    fn ocall_run_new_task(ret: *mut i32) -> sgx_status_t;
    fn do_run_task(task: *mut Task) -> i32;
}
