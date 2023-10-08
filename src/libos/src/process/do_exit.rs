use crate::process::do_vfork::reap_zombie_child_created_with_vfork;
use crate::signal::constants::*;
use std::intrinsics::atomic_store_seqcst;

use super::do_futex::futex_wake;
use super::do_vfork::{is_vforked_child_process, vfork_return_to_parent};
use super::pgrp::clean_pgrp_when_exit;
use super::process::{Process, ProcessFilter};
use super::{table, ProcessRef, TermStatus, ThreadRef, ThreadStatus};
use crate::ipc::SHM_MANAGER;
use crate::prelude::*;
use crate::signal::{KernelSignal, SigNum};
use crate::syscall::CpuContext;
use crate::vm::USER_SPACE_VM_MANAGER;

pub fn do_exit_group(status: i32, curr_user_ctxt: &mut CpuContext) -> Result<isize> {
    if is_vforked_child_process() {
        let current = current!();
        let child_exit_status = TermStatus::Exited(status as u8);
        return vfork_return_to_parent(curr_user_ctxt as *mut _, &current, Some(child_exit_status));
    } else {
        let term_status = TermStatus::Exited(status as u8);
        current!().process().force_exit(term_status);

        // wake all child threads which are waiting
        let _ = current!()
            .process()
            .inner()
            .waiting_children_mut()
            .unwrap()
            .del_and_wake_all_waiters();

        exit_thread(term_status);
        Ok(0)
    }
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
            atomic_store_seqcst(ctid_ptr.as_ptr(), 0);
        }
        futex_wake(ctid_ptr.as_ptr() as *const i32, 1);
    }
    thread.set_clear_ctid(None);

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

    // Notify a thread, if any, that wait on this thread to exit.
    // E.g. In execve, the new thread should wait for old process's all thread to exit
    futex_wake(Arc::as_ptr(&thread.process()) as *const i32, 1);
}

fn exit_process(thread: &ThreadRef, term_status: TermStatus) {
    let process = thread.process();
    let pid = process.pid();

    // Deadlock note: always lock parent first, then child.

    // Lock the idle process since it may adopt new children.
    let idle_ref = super::IDLE.process().clone();
    let mut idle_inner = idle_ref.inner();
    // Lock the parent process as we want to prevent race conditions between
    // current's exit() and parent's wait5().
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
    // Clean used VM
    USER_SPACE_VM_MANAGER.free_chunks_when_exit(thread);
    SHM_MANAGER.detach_shm_when_process_exit(thread);

    // The parent is the idle process
    if parent_inner.is_none() {
        debug_assert!(parent.pid() == 0);

        let main_tid = pid;
        table::del_thread(main_tid).expect("tid must be in the table");
        table::del_process(pid).expect("pid must be in the table");
        clean_pgrp_when_exit(process);

        process_inner.exit(term_status, &idle_ref, &mut idle_inner);

        // For vfork-and-exit children, just clean them to free the pid
        let _ = reap_zombie_child_created_with_vfork(pid);

        idle_inner.remove_zombie_child(pid);
        return;
    }
    // Otherwise, we need to notify the parent process
    let mut parent_inner = parent_inner.unwrap();

    process_inner.exit(term_status, &idle_ref, &mut idle_inner);

    // For vfork-and-exit children, just clean them to free the pid
    let _ = reap_zombie_child_created_with_vfork(pid);

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
    let mut sig_queues = parent.sig_queues().write().unwrap();
    sig_queues.enqueue(signal);
}

pub fn exit_old_process_for_execve(term_status: TermStatus, new_parent_ref: ProcessRef) {
    let thread = current!();

    // Exit current thread
    let num_remaining_threads = thread.exit(term_status);
    if thread.tid() != thread.process().pid() {
        // Keep the main thread's tid available as long as the process is not destroyed.
        // Main thread doesn't need to delete here. It will be repalced later.
        table::del_thread(thread.tid()).expect("tid must be in the table");
    }

    debug_assert!(num_remaining_threads == 0);
    exit_process_for_execve(&thread, new_parent_ref, term_status);
}

// Let new parent process to adopt current process' children
fn exit_process_for_execve(
    thread: &ThreadRef,
    new_parent_ref: ProcessRef,
    term_status: TermStatus,
) {
    let process = thread.process();

    // Deadlock note: always lock parent first, then child.
    // Lock the idle process since it may adopt new children.
    let idle_ref = super::IDLE.process().clone();
    let mut idle_inner = idle_ref.inner();

    // Lock the parent process as we want to prevent race conditions between
    // current's exit() and parent's wait5().
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
    // Clean used VM
    USER_SPACE_VM_MANAGER.free_chunks_when_exit(thread);

    let mut new_parent_inner = new_parent_ref.inner();
    let pid = process.pid();

    // Let new_process to adopt the children of current process
    process_inner.exit(term_status, &new_parent_ref, &mut new_parent_inner);

    // For vfork-and-exit children, we don't need to adopt them here.
    // Because the new parent process share the same pid with the old parent process.

    // Remove current process from parent process' zombie list.
    if parent_inner.is_none() {
        debug_assert!(parent.pid() == 0);
        idle_inner.remove_zombie_child(pid);
        return;
    }

    parent_inner.unwrap().remove_zombie_child(pid);
}
