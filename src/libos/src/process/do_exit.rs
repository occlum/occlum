use crate::signal::constants::*;
use std::intrinsics::atomic_store;

use super::do_futex::futex_wake;
use super::process::{Process, ProcessFilter};
use super::{table, TermStatus, ThreadRef, ThreadStatus};
use crate::prelude::*;
use crate::signal::{KernelSignal, SigNum};

pub fn do_exit_group(status: i32) {
    let term_status = TermStatus::Exited(status as u8);
    current!().process().force_exit(term_status);
    exit_thread(term_status);
}

pub fn do_exit(status: i32) {
    let term_status = TermStatus::Exited(status as u8);
    exit_thread(term_status);
}

/// Exit this thread if its has been forced to exit.
///
/// A thread may be forced to exit for two reasons: 1) a fatal signal; 2)
/// exit_group syscall.
pub fn handle_force_exit() {
    if let Some(term_status) = current!().process().is_forced_exit() {
        exit_thread(term_status);
    }
}

fn exit_thread(term_status: TermStatus) {
    let thread = current!();
    if thread.status() == ThreadStatus::Exited {
        return;
    }

    let num_remaining_threads = thread.exit(term_status);

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
        exit_process(&thread, term_status);
    }
}

fn exit_process(thread: &ThreadRef, term_status: TermStatus) {
    let process = thread.process();

    // If the parent process is the idle process, we can release the process directly.
    if process.parent().pid() == 0 {
        // Deadlock note: Always lock parent then child.
        let mut parent_inner = super::IDLE.process().inner();
        let mut process_inner = process.inner();

        let pid = process.pid();
        let main_tid = pid;
        table::del_thread(main_tid).expect("tid must be in the table");
        table::del_process(pid).expect("pid must be in the table");

        process_inner.exit(term_status);
        parent_inner.remove_zombie_child(process.pid());
        return;
    }
    // Otherwise, we need to notify the parent process

    // Lock the parent process to ensure that parent's wait4 cannot miss the current
    // process's exit.
    // Deadlock note: Always lock parent then child.
    let parent = process.parent();
    let mut parent_inner = parent.inner();
    process.inner().exit(term_status);

    //Send SIGCHLD to parent
    send_sigchld_to(&parent);

    // Wake up the parent if it is waiting on this child
    let waiting_children = parent_inner.waiting_children_mut().unwrap();
    waiting_children.del_and_wake_one_waiter(|waiter_data| -> Option<pid_t> {
        match waiter_data {
            ProcessFilter::WithAnyPid => {}
            ProcessFilter::WithPid(required_pid) => {
                if process.pid() != *required_pid {
                    return None;
                }
            }
            ProcessFilter::WithPgid(required_pgid) => {
                if process.pgid() != *required_pgid {
                    return None;
                }
            }
        }
        Some(process.pid())
    });
}

fn send_sigchld_to(parent: &Arc<Process>) {
    let signal = Box::new(KernelSignal::new(SigNum::from(SIGCHLD)));
    let mut sig_queues = parent.sig_queues().lock().unwrap();
    sig_queues.enqueue(signal);
}
