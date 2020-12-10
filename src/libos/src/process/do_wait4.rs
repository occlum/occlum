use std::sync::Weak;

use super::process::{ProcessFilter, ProcessInner};
use super::{table, ProcessRef, ProcessStatus};
use crate::events::{Observer, Waiter};
use crate::prelude::*;

pub fn do_wait4(child_filter: &ProcessFilter) -> Result<(pid_t, i32)> {
    let thread = current!();
    let process = thread.process();

    let waiter = Waiter::new();
    loop {
        process.observer().waiter_queue().reset_and_enqueue(&waiter);

        // Lock order: always lock parent then child to avoid deadlock
        let mut process_inner = process.inner();

        let unwaited_children = process_inner
            .children()
            .unwrap()
            .iter()
            .filter(|child| match child_filter {
                ProcessFilter::WithAnyPid => true,
                ProcessFilter::WithPid(required_pid) => child.pid() == *required_pid,
                ProcessFilter::WithPgid(required_pgid) => child.pgid() == *required_pgid,
            })
            .collect::<Vec<&ProcessRef>>();

        if unwaited_children.len() == 0 {
            return_errno!(ECHILD, "Cannot find any unwaited children");
        }

        // Return immediately if a child that we wait for has already exited
        let zombie_child = unwaited_children
            .iter()
            .find(|child| child.status() == ProcessStatus::Zombie);
        if let Some(zombie_child) = zombie_child {
            let zombie_pid = zombie_child.pid();
            let exit_status = free_zombie_child(&process, process_inner, zombie_pid);
            return Ok((zombie_pid, exit_status));
        }

        drop(process_inner);

        waiter.wait(None);
    }
}

fn free_zombie_child(
    parent: &ProcessRef,
    mut parent_inner: SgxMutexGuard<ProcessInner>,
    zombie_pid: pid_t,
) -> i32 {
    // Remove zombie from the process and thread table
    table::del_thread(zombie_pid).expect("tid must be in the table");
    table::del_process(zombie_pid).expect("pid must be in the table");

    let zombie = parent_inner.remove_zombie_child(zombie_pid);
    debug_assert!(zombie.status() == ProcessStatus::Zombie);

    let observer = Arc::downgrade(parent.observer()) as Weak<dyn Observer<_>>;
    zombie.notifier().unregister(&observer);

    let zombie_inner = zombie.inner();
    zombie_inner.term_status().unwrap().as_u32() as i32
}
