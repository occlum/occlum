use std::intrinsics::atomic_store;

use super::do_futex::futex_wake;
use super::process::ChildProcessFilter;
use super::{table, ThreadRef};
use crate::prelude::*;

pub fn do_exit(exit_status: i32) {
    let thread = current!();

    let num_remaining_threads = thread.exit(exit_status);

    // Notify a thread, if any, that waits on ctid. See set_tid_address(2) for more info.
    if let Some(ctid_ptr) = thread.clear_ctid() {
        unsafe {
            atomic_store(ctid_ptr.as_ptr(), 0);
        }
        futex_wake(ctid_ptr.as_ptr() as *const i32, 1);
    }

    // Keep the main thread's tid available as long as the process is not destroyed.
    // This is important as the user space may still attempt to access the main
    // thread's ThreadRef through the process's pid after the process has become
    // a zombie.
    if thread.tid() != thread.process().pid() {
        table::del_thread(thread.tid()).expect("tid must be in the table");
    }

    // If this thread is the last thread, then exit the process
    if num_remaining_threads == 0 {
        do_exit_process(&thread, exit_status);
    }
}

fn do_exit_process(thread: &ThreadRef, exit_status: i32) {
    let process = thread.process();

    // If the parent process is the idle process, we can release the process directly.
    if process.parent().pid() == 0 {
        // Deadlock note: Always lock parent then child.
        let mut parent_inner = super::IDLE.process().inner();
        let mut process_inner = process.inner();

        table::del_thread(thread.tid()).expect("tid must be in the table");
        table::del_process(process.pid()).expect("pid must be in the table");

        process_inner.exit(exit_status);
        parent_inner.remove_zombie_child(process.pid());
        return;
    }
    // Otherwise, we need to notify the parent process

    // Lock the parent process to ensure that parent's wait4 cannot miss the current
    // process's exit.
    // Deadlock note: Always lock parent then child.
    let parent = process.parent();
    let mut parent_inner = parent.inner();
    process.inner().exit(exit_status);

    // Wake up the parent if it is waiting on this child
    let waiting_children = parent_inner.waiting_children_mut().unwrap();
    waiting_children.del_and_wake_one_waiter(|waiter_data| -> Option<pid_t> {
        match waiter_data {
            ChildProcessFilter::WithAnyPid => {}
            ChildProcessFilter::WithPid(required_pid) => {
                if process.pid() != *required_pid {
                    return None;
                }
            }
            ChildProcessFilter::WithPgid(required_pgid) => {
                if process.pgid() != *required_pgid {
                    return None;
                }
            }
        }
        Some(process.pid())
    });
}
