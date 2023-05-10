use std::sync::Weak;

use async_rt::waiter_loop;

use super::do_vfork::wait4_exit_child_created_with_vfork;
use super::pgrp::clean_pgrp_when_exit;
use super::process::{ProcessFilter, ProcessInner};
use super::{table, ProcessRef, ProcessStatus};
use crate::prelude::*;

// Children process exits without parent calls wait4 should be reaped by Idle process in the end.
// Without this, there might be memory leakage when exit.
pub fn idle_reap_zombie_children() {
    let idle_ref = super::IDLE.process().clone();
    let mut zombie_pids = Vec::new();
    loop {
        // This needs to acquire lock every time.
        let mut idle_inner = idle_ref.inner();
        let children = idle_inner.children().unwrap();
        match children
            .iter()
            .find(|child| child.status() == ProcessStatus::Zombie)
        {
            Some(zombie_child) => {
                // Reap one zombie each time.
                let zombie_pid = zombie_child.pid();
                let exit_status = free_zombie_child(&idle_ref, idle_inner, zombie_pid);
                zombie_pids.push(zombie_pid);
            }
            None => {
                // None zombie child, just return
                break;
            }
        }
    }

    info!("Idle process reaps zombie children pid = {:?}", zombie_pids);
}

pub async fn do_wait4(child_filter: &ProcessFilter, options: WaitOptions) -> Result<(pid_t, i32)> {
    let thread = current!();
    let process = thread.process();

    let mut timeout = None::<core::time::Duration>;
    waiter_loop!(process.exit_waiters(), timeout, false, {
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

        // Return immediately if a child that we wait for has already exited
        let zombie_child = unwaited_children
            .iter()
            .find(|child| child.status() == ProcessStatus::Zombie);
        if let Some(zombie_child) = zombie_child {
            let zombie_pid = zombie_child.pid();
            let exit_status = free_zombie_child(&process, process_inner, zombie_pid);
            return Ok((zombie_pid, exit_status));
        }

        // Check again for vfork-and-exit child process which doesn't have a real structure of a process
        if let Some(child_status) = wait4_exit_child_created_with_vfork(process.pid(), child_filter)
        {
            return Ok(child_status);
        } else if unwaited_children.len() == 0 {
            // No unwaited children or vforked children, return immediately
            return_errno!(ECHILD, "Cannot find any unwaited children");
        }

        // TODO: Support these options
        if !options.supported() {
            warn!("Unsupported options contained. wait options: {:?}", options);
        }

        // If the WNOHANG bit is set in OPTIONS, and that child
        // is not already dead, return (pid_t) 0.  If successful,
        // return PID and store the dead child's status in STAT_LOC.
        if options.contains(WaitOptions::WNOHANG) {
            return Ok((0, 0));
        }

        // The wait4 is interrupted by exit_group from other threads. In this case, just return.
        if process.is_forced_to_exit() {
            return_errno!(EINTR, "wait is interrupted and not get any children");
        }
    })
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

    // This has to be done after removing from process table to make sure process.pgid() can work.
    clean_pgrp_when_exit(&zombie);

    let zombie_inner = zombie.inner();
    zombie_inner.term_status().unwrap().as_u32() as i32
}

// Based on waitflags.h
// WNOWAIT is not listed here which can only be used in "waitid" syscall.
bitflags! {
    pub struct WaitOptions: u32 {
        const WNOHANG = 0x1;
        //Note: Below flags are not supported yet
        const WSTOPPED = 0x2; // Same as WUNTRACED
        const WEXITED = 0x4;
        const WCONTINUED = 0x8;
    }
}

impl WaitOptions {
    fn supported(&self) -> bool {
        let unsupported_flags = WaitOptions::all() - WaitOptions::WNOHANG;
        !self.intersects(unsupported_flags)
    }
}

// Based on waitstatus.h
const WAIT_STATUS_STOPPED: i32 = 0x7f;
const WAIT_STATUS_CONTINUED: i32 = 0xffff;
