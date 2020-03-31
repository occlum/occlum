use super::*;
use std::intrinsics::atomic_store;

// TODO: make sure Processes are released eventually

#[derive(Clone, Copy, Debug)]
pub enum ChildProcessFilter {
    WithAnyPID,
    WithPID(pid_t),
    WithPGID(pid_t),
}

unsafe impl Send for ChildProcessFilter {}

pub fn do_exit(exit_status: i32) {
    let current_ref = get_current();
    let mut current = current_ref.lock().unwrap();
    let parent_ref = current.get_parent().clone();
    // Update current
    current.exit_status = exit_status;
    current.status = Status::ZOMBIE;

    // Update children
    for child_ref in current.get_children_iter() {
        let mut child = child_ref.lock().unwrap();
        child.parent = Some(IDLE_PROCESS.clone());
    }
    current.children.clear();

    // Notify another process, if any, that waits on ctid (see set_tid_address)
    if let Some(ctid) = current.clear_child_tid {
        unsafe {
            atomic_store(ctid, 0);
        }
        futex_wake(ctid as *const i32, 1);
    }

    // If the process is detached, no need to notify the parent
    if current.is_detached {
        let current_tid = current.get_tid();
        drop(current);
        remove_zombie_child(&parent_ref, current_tid);
        return;
    }

    // Notify the parent process if necessary
    let (mut parent, current) = {
        // Always lock parent before its child
        drop(current);
        lock_two_in_order(&parent_ref, &current_ref)
    };
    // Wake up the parent if it is waiting on this child
    if parent.waiting_children.is_none() {
        return;
    }
    let mut wait_queue = parent.waiting_children.as_mut().unwrap();
    wait_queue.del_and_wake_one_waiter(|waiter_data| -> Option<pid_t> {
        match waiter_data {
            ChildProcessFilter::WithAnyPID => {}
            ChildProcessFilter::WithPID(required_pid) => {
                if current.get_pid() != *required_pid {
                    return None;
                }
            }
            ChildProcessFilter::WithPGID(required_pgid) => {
                if current.get_pgid() != *required_pgid {
                    return None;
                }
            }
        }
        Some(current.get_pid())
    });
}

pub fn do_wait4(child_filter: &ChildProcessFilter, exit_status: &mut i32) -> Result<pid_t> {
    let current_ref = get_current();
    let waiter = {
        let mut current = current_ref.lock().unwrap();

        let mut any_child_to_wait_for = false;
        for child_ref in current.get_children_iter() {
            let child = child_ref.lock().unwrap();

            let may_wait_for = match child_filter {
                ChildProcessFilter::WithAnyPID => true,
                ChildProcessFilter::WithPID(required_pid) => child.get_pid() == *required_pid,
                ChildProcessFilter::WithPGID(required_pgid) => child.get_pgid() == *required_pgid,
            };
            if !may_wait_for {
                continue;
            }

            // Return immediately as a child that we wait for has already exited
            if child.status == Status::ZOMBIE {
                process_table::remove(child.pid);
                return Ok(child.pid);
            }

            any_child_to_wait_for = true;
        }
        if !any_child_to_wait_for {
            return_errno!(ECHILD, "No such child");
        }

        let waiter = Waiter::new(child_filter);
        let mut wait_queue = WaitQueue::new();
        wait_queue.add_waiter(&waiter);

        current.waiting_children = Some(wait_queue);

        waiter
    };

    // Wait until a child has interesting events
    let child_pid = waiter.sleep_until_woken_with_result();

    // Remove the child from the parent
    *exit_status = remove_zombie_child(&current_ref, child_pid);

    let mut current = current_ref.lock().unwrap();
    current.waiting_children = None;

    Ok(child_pid)
}

fn remove_zombie_child(parent_ref: &ProcessRef, child_tid: pid_t) -> i32 {
    // Find the zombie child process
    let mut parent = parent_ref.lock().unwrap();
    let (child_i, child_ref) = parent
        .get_children_iter()
        .enumerate()
        .find(|(child_i, child_ref)| {
            let child = child_ref.lock().unwrap();
            if child.get_tid() != child_tid {
                return false;
            }
            assert!(child.get_status() == Status::ZOMBIE);
            true
        })
        .expect("cannot find the zombie child");

    // Remove the zombie child from parent
    parent.children.swap_remove(child_i);
    // Remove the zombie child from process table
    process_table::remove(child_tid);

    // Return the exit status
    let child = child_ref.lock().unwrap();
    child.get_exit_status()
}

fn lock_two_in_order<'a>(
    first_ref: &'a ProcessRef,
    second_ref: &'a ProcessRef,
) -> (SgxMutexGuard<'a, Process>, SgxMutexGuard<'a, Process>) {
    (first_ref.lock().unwrap(), second_ref.lock().unwrap())
}
