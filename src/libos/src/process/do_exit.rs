use std::intrinsics::atomic_store;
use std::sync::Weak;

use super::do_futex::futex_wake;
use super::process::{Process, ProcessFilter};
use super::{table, ProcessRef, TermStatus, ThreadRef, ThreadStatus};
use crate::prelude::*;
use crate::signal::constants::*;
use crate::signal::{KernelSignal, SigNum};

pub fn do_exit_group(status: i32) {
    let term_status = TermStatus::Exited(status as u8);
    let current = current!();
    current.process().force_exit(term_status);
    exit_thread(term_status);

    // Interrupt all threads in the process to ensure that they exit
    current.process().access_threads_with(|thread| {
        let task = match thread.task() {
            Some(task) => task,
            None => return,
        };

        use crate::signal::SIGKILL;
        task.tirqs().put_req(SIGKILL.as_tirq_line());
    });
}

pub fn do_exit(status: i32) {
    let term_status = TermStatus::Exited(status as u8);
    exit_thread(term_status);
}

/// Exit this thread if it has been forced to exit.
///
/// A thread may be forced to exit for two reasons: 1) a fatal signal; 2)
/// exit_group syscall.
pub fn handle_force_exit() {
    if current!().process().is_forced_to_exit() {
        exit_thread(current!().process().term_status().unwrap());
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

    // Notify waiters that the owner of robust futex has died.
    thread.wake_robust_list();

    // Keep the main thread's tid available as long as the process is not destroyed.
    // This is important as the user space may still attempt to access the main
    // thread's ThreadRef through the process's pid after the process has become
    // a zombie.
    if thread.tid() != thread.process().pid() {
        table::del_thread(thread.tid()).expect("tid must be in the table");
    }

    // If this thread is the last thread, close all files then exit the process
    if num_remaining_threads == 0 {
        thread.close_all_files();
        exit_process(&thread, term_status);
    }
}

fn exit_process(thread: &ThreadRef, term_status: TermStatus) {
    let process = thread.process();

    // Deadlock note: always lock parent first, then child.

    // Lock the idle process since it may adopt new children.
    let idle_ref = super::IDLE.process().clone();
    let mut idle_inner = idle_ref.inner();
    // Lock the parent process as we want to prevent race conditions between
    // current's exit() and parent's wait4().
    let mut parent;
    let mut parent_inner = loop {
        parent = process.parent();
        if parent.pid() == 0 {
            // If the parent is the idle process, don't need to lock again
            break None;
        }

        let parent_inner = parent.inner();
        // To prevent the race condition that parent is changed after `parent()`,
        // but before `parent().innner()`, we need to check again here.
        if parent.pid() != process.parent().pid() {
            continue;
        }
        break Some(parent_inner);
    };
    // Lock the current process
    let mut process_inner = process.inner();

    // The parent is the idle process
    if parent_inner.is_none() {
        debug_assert!(parent.pid() == 0);

        let pid = process.pid();
        let main_tid = pid;
        table::del_thread(main_tid).expect("tid must be in the table");
        table::del_process(pid).expect("pid must be in the table");

        process_inner.exit(term_status, &idle_ref, &mut idle_inner, &parent);
        idle_inner.remove_zombie_child(pid);
        wake_host(&process, term_status);
        return;
    }
    // Otherwise, we need to notify the parent process
    let mut parent_inner = parent_inner.unwrap();
    process_inner.exit(term_status, &idle_ref, &mut idle_inner, &parent);

    //Send SIGCHLD to parent
    send_sigchld_to(&parent);

    drop(idle_inner);
    drop(parent_inner);
    drop(process_inner);

    // Notify the parent that this child process's status has changed
    parent.exit_waiters().wake_all();

    // Notify the host threads that wait the status change of this process
    wake_host(&process, term_status);
}

fn send_sigchld_to(parent: &Arc<Process>) {
    let signal = Box::new(KernelSignal::new(SigNum::from(SIGCHLD)));
    let mut sig_queues = parent.sig_queues().write().unwrap();
    sig_queues.enqueue(signal);
}

fn wake_host(process: &ProcessRef, term_status: TermStatus) {
    if let Some(host_waker) = process.host_waker() {
        host_waker.wake(term_status);
    }
}
