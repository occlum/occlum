use std::mem;

use super::*;

/// Note: this definition must be in sync with task.h
#[derive(Clone, Debug, Default)]
#[repr(C)]
pub struct Task {
    kernel_rsp: usize,
    kernel_stack_base: usize,
    kernel_stack_limit: usize,
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
    ) -> Result<Task> {
        if !(user_stack_base >= user_rsp && user_rsp > user_stack_limit) {
            return_errno!(EINVAL, "Invalid user stack");
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
    static ref NEW_PROCESS_TABLE: SgxMutex<HashMap<pid_t, ProcessRef>> =
        { SgxMutex::new(HashMap::new()) };
}

pub fn enqueue_task(new_tid: pid_t, new_process: ProcessRef) {
    let existing_task = NEW_PROCESS_TABLE
        .lock()
        .unwrap()
        .insert(new_tid, new_process);
    // There should NOT have any pending process with the same ID
    assert!(existing_task.is_none());
}

pub fn enqueue_and_exec_task(new_tid: pid_t, new_process: ProcessRef) {
    enqueue_task(new_tid, new_process);

    let mut ret = 0;
    let ocall_status = unsafe { occlum_ocall_exec_thread_async(&mut ret, new_tid) };
    if ocall_status != sgx_status_t::SGX_SUCCESS || ret != 0 {
        panic!("Failed to start the process");
    }
}

fn dequeue_task(libos_tid: pid_t) -> Result<ProcessRef> {
    NEW_PROCESS_TABLE
        .lock()
        .unwrap()
        .remove(&libos_tid)
        .ok_or_else(|| errno!(EAGAIN, "the given TID does not match any pending process"))
}

pub fn run_task(libos_tid: pid_t, host_tid: pid_t) -> Result<i32> {
    let new_process: ProcessRef = dequeue_task(libos_tid)?;
    set_current(&new_process);

    let (pid, task) = {
        let mut process = new_process.lock().unwrap();
        process.set_host_tid(host_tid);
        let pid = process.get_pid();
        let task = process.get_task_mut() as *mut Task;
        (pid, task)
    };

    #[cfg(feature = "syscall_timing")]
    GLOBAL_PROFILER
        .lock()
        .unwrap()
        .thread_enter()
        .expect("unexpected error from profiler to enter thread");

    unsafe {
        // task may only be modified by this function; so no lock is needed
        do_run_task(task);
    }

    #[cfg(feature = "syscall_timing")]
    GLOBAL_PROFILER
        .lock()
        .unwrap()
        .thread_exit()
        .expect("unexpected error from profiler to exit thread");

    let (exit_status, parent_pid) = {
        let mut process = new_process.lock().unwrap();
        let parent = process.get_parent().lock().unwrap();
        (process.get_exit_status(), parent.get_tid())
    };

    // If process's parent is the IDLE_PROCESS (pid = 0), so it has to release itself
    if parent_pid == 0 {
        process_table::remove(pid);
    }

    reset_current();
    Ok(exit_status)
}

thread_local! {
    static _CURRENT_PROCESS_PTR: Cell<*const SgxMutex<Process>> = {
        Cell::new(0 as *const SgxMutex<Process>)
    };
    // for log getting pid without locking process
    static _TID: Cell<pid_t> = Cell::new(0);
}

pub fn get_current_tid() -> pid_t {
    _TID.with(|tid_cell| tid_cell.get())
}

pub fn get_current() -> ProcessRef {
    let current_ptr = _CURRENT_PROCESS_PTR.with(|cell| cell.get());

    let current_ref = unsafe { Arc::from_raw(current_ptr) };
    let current_ref_clone = current_ref.clone();
    Arc::into_raw(current_ref);

    current_ref_clone
}

fn set_current(process: &ProcessRef) {
    let tid = process.lock().unwrap().get_tid();
    _TID.with(|tid_cell| tid_cell.set(tid));

    let process_ref_clone = process.clone();
    let process_ptr = Arc::into_raw(process_ref_clone);

    _CURRENT_PROCESS_PTR.with(|cp| {
        cp.set(process_ptr);
    });
}

fn reset_current() {
    _TID.with(|tid_cell| tid_cell.set(0));
    let mut process_ptr = _CURRENT_PROCESS_PTR.with(|cp| cp.replace(0 as *const SgxMutex<Process>));

    // Prevent memory leakage
    unsafe {
        drop(Arc::from_raw(process_ptr));
    }
}

extern "C" {
    fn occlum_ocall_exec_thread_async(ret: *mut i32, libos_tid: pid_t) -> sgx_status_t;
    fn do_run_task(task: *mut Task) -> i32;
}
