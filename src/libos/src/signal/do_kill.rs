use super::constants::*;
use super::signals::{KernelSignal, UserSignal, UserSignalKind};
use super::{SigNum, Signal};
use crate::prelude::*;
use crate::process::{table, ProcessFilter, ProcessRef, ProcessStatus, ThreadRef, ThreadStatus};

pub fn do_kill(filter: ProcessFilter, signum: SigNum) -> Result<()> {
    debug!("do_kill: filter: {:?}, signum: {:?}", &filter, &signum);

    let pid = current!().process().pid();
    let uid = 0;
    let processes = get_processes(&filter)?;
    for process in processes {
        if process.status() == ProcessStatus::Zombie {
            continue;
        }

        let signal = Box::new(UserSignal::new(signum, UserSignalKind::Kill, pid, uid));
        let mut sig_queues = process.sig_queues().write().unwrap();
        sig_queues.enqueue(signal);
    }
    Ok(())
}

/// Send a signal from the outside the enclave.
///
/// Such a call must be performed very carefully. The obvious reason
/// is that the call is not trusted. And there is a less obvious reason:
/// the function is not executed during a normal syscall. Thus, current!() does
/// not refer to a valid LibOS thread. So let's implement this function with
/// these two insights in mind.
pub fn do_kill_from_outside_enclave(filter: ProcessFilter, signum: SigNum) -> Result<()> {
    let signal = {
        if signum != SIGKILL && signum != SIGTERM {
            return_errno!(EPERM, "The signal is not allowed");
        }
        Box::new(KernelSignal::new(signum))
    };
    let processes = get_processes(&filter)?;
    for process in processes {
        if process.status() == ProcessStatus::Zombie {
            continue;
        }

        let mut sig_queues = process.sig_queues().write().unwrap();
        sig_queues.enqueue(signal.clone());
    }
    Ok(())
}

fn get_processes(filter: &ProcessFilter) -> Result<Vec<ProcessRef>> {
    let processes = match filter {
        ProcessFilter::WithAnyPid => table::get_all_processes(),
        ProcessFilter::WithPid(pid) => {
            let process = table::get_process(*pid)?;
            vec![process]
        }
        ProcessFilter::WithPgid(pgid) => {
            let pgrp = table::get_pgrp(*pgid)?;
            let processes = pgrp.get_all_processes();
            if processes.len() == 0 {
                return_errno!(EINVAL, "invalid pgid");
            }
            processes
        }
    };
    Ok(processes)
}

pub fn do_tgkill(pid: Option<pid_t>, tid: pid_t, signum: SigNum) -> Result<()> {
    debug!(
        "do_tgkill: pid: {:?}, tid: {:?}, signum: {:?}",
        &pid, &tid, &signum
    );

    let thread = table::get_thread(tid)?;
    if let Some(pid) = pid {
        if pid != thread.process().pid() {
            return_errno!(EINVAL, "the combination of pid and tid is not valid");
        }
    }

    if thread.status() == ThreadStatus::Exited {
        return Ok(());
    }

    let signal = {
        let src_pid = current!().process().pid();
        let src_uid = 0;
        Box::new(UserSignal::new(
            signum,
            UserSignalKind::Tkill,
            src_pid,
            src_uid,
        ))
    };
    let mut sig_queues = thread.sig_queues().write().unwrap();
    sig_queues.enqueue(signal);
    Ok(())
}
