use super::process::{ProcessFilter, ProcessInner};
use super::wait::Waiter;
use super::{table, ProcessRef, ProcessStatus};
use crate::prelude::*;

pub fn do_wait4(child_filter: &ProcessFilter) -> Result<(pid_t, i32)> {
    // Lock the process early to ensure that we do not miss any changes in
    // children processes
    let thread = current!();
    let process = thread.process();
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
        let exit_status = free_zombie_child(process_inner, zombie_pid);
        return Ok((zombie_pid, exit_status));
    }

    let mut waiter = Waiter::new(child_filter);
    process_inner
        .waiting_children_mut()
        .unwrap()
        .add_waiter(&waiter);
    // After adding the waiter, we can safely release the lock on the process inner
    // without risking missing events from the process's children.
    drop(process_inner);
    // Wait until a child has interesting events
    let zombie_pid = waiter.sleep_until_woken_with_result();

    let mut process_inner = process.inner();
    let exit_status = free_zombie_child(process_inner, zombie_pid);
    Ok((zombie_pid, exit_status))
}

fn free_zombie_child(mut parent_inner: SgxMutexGuard<ProcessInner>, zombie_pid: pid_t) -> i32 {
    // Remove zombie from the process and thread table
    table::del_thread(zombie_pid).expect("tid must be in the table");
    table::del_process(zombie_pid).expect("pid must be in the table");

    let zombie = parent_inner.remove_zombie_child(zombie_pid);
    debug_assert!(zombie.status() == ProcessStatus::Zombie);

    let zombie_inner = zombie.inner();
    zombie_inner.term_status().unwrap().as_u32() as i32
}
