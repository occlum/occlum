use std::mem;

use super::*;

/// Note: this definition must be in sync with task.h
#[derive(Clone, Debug, Default)]
#[repr(C)]
pub struct Task {
    kernel_rsp: usize,
    kernel_fs: usize,
    user_rsp: usize,
    user_stack_base: usize,
    user_stack_limit: usize,
    user_fs: usize,
    user_entry_addr: usize,
    saved_state: usize, // struct jmpbuf*
}

impl Task {
    pub unsafe fn new(
        user_entry_addr: usize,
        user_rsp: usize,
        user_stack_base: usize,
        user_stack_limit: usize,
        user_fs: Option<usize>,
    ) -> Result<Task, Error> {
        if !(user_stack_base >= user_rsp && user_rsp > user_stack_limit) {
            return errno!(EINVAL, "Invalid user stack");
        }

        // Set the default user fsbase to an address on user stack, which is
        // a relatively safe address in case the user program uses %fs before
        // initializing fs base address.
        let user_fs = user_fs.unwrap_or(user_stack_limit);

        Ok(Task {
            user_entry_addr,
            user_rsp,
            user_stack_base,
            user_stack_limit,
            user_fs,
            ..Default::default()
        })
    }

    pub fn set_user_fs(&mut self, user_fs: usize) {
        self.user_fs = user_fs;
    }

    pub fn get_user_fs(&self) -> usize {
        self.user_fs
    }
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
    // for log getting pid without locking process
    static _PID: Cell<pid_t> = Cell::new(0);
}

pub fn current_pid() -> pid_t {
    _PID.with(|p| p.get())
}

pub fn get_current() -> ProcessRef {
    let current_ptr = _CURRENT_PROCESS_PTR.with(|cell| cell.get());

    let current_ref = unsafe { Arc::from_raw(current_ptr) };
    let current_ref_clone = current_ref.clone();
    Arc::into_raw(current_ref);

    current_ref_clone
}

fn set_current(process: &ProcessRef) {
    let pid = process.lock().unwrap().get_pid();
    _PID.with(|p| p.set(pid));

    let process_ref_clone = process.clone();
    let process_ptr = Arc::into_raw(process_ref_clone);

    _CURRENT_PROCESS_PTR.with(|cp| {
        cp.set(process_ptr);
    });
}

fn reset_current() {
    _PID.with(|p| p.set(0));
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
