use std::ffi::{CStr, CString};
use std::path::Path;

use super::do_exit::{exit_old_process_for_execve, notify_all_threads_to_exit};
use super::do_spawn::new_process_for_exec;
use super::process::ProcessFilter;
use super::term_status::TermStatus;
use super::ThreadRef;
use super::{table, ProcessRef, ProcessStatus};
use crate::prelude::*;

pub async fn do_exec(
    path: &str,
    argv: &[CString],
    envp: &[CString],
    current_ref: &ThreadRef,
) -> Result<isize> {
    trace!(
        "exec current process pid = {:?}",
        current_ref.process().pid()
    );

    // Construct new process structure but with same parent, pid, tid
    let current = current!();
    let ret = super::do_spawn::new_process_for_exec(path, argv, envp, current_ref);

    if let Ok((new_process_ref, init_cpu_state)) = ret {
        let new_main_thread = new_process_ref
            .main_thread()
            .expect("the main thread is just created; it must exist");

        // Force exit all child threads of current process
        let term_status = TermStatus::Exited(0 as u8);
        current.process().force_exit(term_status);

        notify_all_threads_to_exit(current.process());

        // Must yield here for other threads to run
        async_rt::sched::yield_().await;

        // Wait for all threads (except calling thread) to exit
        wait_for_other_threads_to_exit(current);

        // Exit current thread and let new process to adopt current's child process
        exit_old_process_for_execve(term_status, new_process_ref.clone());

        // Update process and thread in global table
        table::replace_process(new_process_ref.pid(), new_process_ref.clone());
        table::replace_thread(
            new_process_ref.pid(),
            new_process_ref.main_thread().unwrap(),
        );

        // pass process signal waiter queue to the task
        let irq_waiters = new_process_ref.sig_waiters();

        // Finally, enqueue the new thread for execution
        async_rt::task::spawn(crate::entry::thread::main_loop(
            new_main_thread,
            init_cpu_state,
        ));
        return Ok(0);
    } else {
        // There is something wrong when constructing new process. Just return the error.
        let error = ret.unwrap_err();
        return Err(error);
    }
}

// Blocking wait until there is only one thread in the calling process
fn wait_for_other_threads_to_exit(current_ref: ThreadRef) {
    use super::do_futex::{self};
    use crate::time::{timespec_t, ClockId};
    use core::time::Duration;

    // Set timeout to 50ms
    let timeout = Duration::from_millis(50);
    // Use calling process's pointer as futex value
    let futex_val = Arc::as_ptr(&current_ref.process()) as *const i32;
    loop {
        let thread_num = current_ref.process().threads().len();
        if thread_num == 1 {
            return;
        }
        // Blocking wait here. When a thread exit, it will notify us.
        unsafe {
            do_futex::futex_wait(
                Arc::as_ptr(&current_ref.process()) as *const i32,
                *futex_val,
                &Some(timeout),
            )
        };
    }
}
