use super::{*};

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

    // Update current
    current.exit_status = exit_status;
    current.status = Status::ZOMBIE;

    // Update children
    for child_ref in &current.children {
        let mut child = child_ref.lock().unwrap();
        child.parent = Some(IDLE_PROCESS.clone());
    }
    current.children.clear();

    // Notify parent if necessary
    let parent_ref = current.get_parent().clone();
    let (mut parent, current) = {
        // Always lock parent before its child
        drop(current);
        lock_two_in_order(&parent_ref, &current_ref)
    };
    // Wake up the parent if it is waiting on this child
    if parent.waiting_children.is_none() { return; }
    let mut wait_queue = parent.waiting_children.as_mut().unwrap();
    wait_queue.del_and_wake_one_waiter(|waiter_data| -> Option<pid_t> {
        match waiter_data {
            ChildProcessFilter::WithAnyPID => {
            },
            ChildProcessFilter::WithPID(required_pid) => {
                if current.get_pid() != *required_pid {
                    return None;
                }
            },
            ChildProcessFilter::WithPGID(required_pgid) => {
                if current.get_pgid() != *required_pgid {
                    return None;
                }
            },
        }
        Some(current.get_pid())
    });
}

pub fn do_wait4(child_filter: &ChildProcessFilter, exit_status: &mut i32)
    -> Result<pid_t, Error>
{
    let waiter = {
        let current_ref = get_current();
        let mut current = current_ref.lock().unwrap();

        let mut any_child_to_wait_for = false;
        for child_ref in current.get_children() {
            let child = child_ref.lock().unwrap();

            let may_wait_for = match child_filter {
                ChildProcessFilter::WithAnyPID => {
                    true
                },
                ChildProcessFilter::WithPID(required_pid) => {
                    child.get_pid() == *required_pid
                },
                ChildProcessFilter::WithPGID(required_pgid) => {
                    child.get_pgid() == *required_pgid
                }
            };
            if !may_wait_for { continue; }

            // Return immediately as a child that we wait for has alreay exited
            if child.status == Status::ZOMBIE {
                return Ok(child.pid);
            }

            any_child_to_wait_for = true;
        }
        if !any_child_to_wait_for { return errno!(ECHILD, "No such child"); }

        let waiter = Waiter::new(child_filter);
        let mut wait_queue = WaitQueue::new();
        wait_queue.add_waiter(&waiter);

        current.waiting_children = Some(wait_queue);

        waiter
    };

    let child_pid = waiter.wait_on();
    if child_pid == 0 { panic!("THIS SHOULD NEVER HAPPEN!"); }

    {
        let current_ref = get_current();
        let mut current = current_ref.lock().unwrap();
        current.waiting_children = None;
    }

    let child_ref = process_table::get(child_pid).unwrap();
    let child = {
        let child = child_ref.lock().unwrap();
        if child.get_status() != Status::ZOMBIE {
            panic!("THIS SHOULD NEVER HAPPEN!");
        }
        child
    };
    *exit_status = child.get_exit_status();
    process_table::remove(child_pid);

    Ok(child_pid)
}

fn lock_two_in_order<'a>(first_ref: &'a ProcessRef, second_ref: &'a ProcessRef) ->
    (SgxMutexGuard<'a, Process>, SgxMutexGuard<'a, Process>)
{
    (first_ref.lock().unwrap(), second_ref.lock().unwrap())
}

