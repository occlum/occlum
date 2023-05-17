use std::ffi::{CStr, CString};
use std::path::Path;

use super::do_exit::{exit_old_process_for_execve, notify_all_threads_to_exit};
use super::do_spawn::new_process_for_exec;
use super::do_vfork::{check_vfork_for_exec, vfork_return_to_parent};
use super::process::ProcessFilter;
use super::term_status::TermStatus;
use super::{table, ProcessRef, ProcessStatus};
use super::{ThreadId, ThreadRef};
use crate::entry::context_switch::CURRENT_CONTEXT;
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

    let mut curr_user_ctxt = CURRENT_CONTEXT.with(|context| context.as_ptr());

    // Check if this process is a vfork-ed child process or a normal process directly calling execve
    let (is_vforked, tid, parent_process) =
        if let Some((tid, parent_process)) = check_vfork_for_exec(current_ref) {
            // It is a vfork-ed child process
            (true, tid, parent_process)
        } else {
            // Current process directly calls execve
            // Construct new process structure but with same parent, pid, tid
            (
                false,
                // Reuse self tid
                ThreadId {
                    tid: current_ref.process().pid() as u32,
                },
                // Reuse parent process as parent
                Some(current_ref.process().parent().clone()),
            )
        };

    let ret =
        super::do_spawn::new_process_for_exec(path, argv, envp, current_ref, tid, parent_process)
            .await;

    if let Ok((new_process_ref, init_cpu_state)) = ret {
        let new_main_thread = new_process_ref
            .main_thread()
            .expect("the main thread is just created; it must exist");

        if is_vforked {
            let irq_waiters = new_process_ref.sig_waiters();

            // Don't exit current process if this is a vforked child process.
            table::add_thread(new_process_ref.main_thread().unwrap());
            table::add_process(new_process_ref);

            // Finally, enqueue the new thread for execution
            async_rt::task::spawn(crate::entry::thread::main_loop(
                new_main_thread,
                init_cpu_state,
            ));

            return vfork_return_to_parent(curr_user_ctxt, current_ref, None).await;
        }

        // Force exit all child threads of current process
        let term_status = TermStatus::Exited(0 as u8);
        current_ref.process().force_exit(term_status);

        // Wait for all threads (except calling thread) to exit
        wait_for_other_threads_to_exit(current_ref).await;

        // Exit current thread and let new process to adopt current's child process
        exit_old_process_for_execve(term_status, new_process_ref.clone()).await;

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
async fn wait_for_other_threads_to_exit(current_ref: &ThreadRef) {
    use super::do_futex::{self};
    use crate::time::{timespec_t, ClockId};
    use core::time::Duration;

    // Set timeout to 50ms
    let timeout = Duration::from_millis(50);
    // Use calling process's pointer as futex value
    let futex_val = Arc::as_ptr(current_ref.process()) as *const i32;
    loop {
        // Do this for every loop in case a new thread is created just after the notification
        notify_all_threads_to_exit(current_ref.process());

        // Must yield here for other threads to run
        async_rt::scheduler::yield_now().await;

        let thread_num = current_ref.process().threads().len();
        if thread_num == 1 {
            return;
        }
        // Blocking wait here. When a thread exit, it will notify us.
        unsafe {
            do_futex::futex_wait(
                Arc::as_ptr(current_ref.process()) as *const i32,
                *futex_val,
                &Some(timeout),
            )
        };
    }
}
