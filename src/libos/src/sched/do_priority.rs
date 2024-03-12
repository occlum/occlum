use super::priority::{NiceValue, PrioWhich};
use crate::prelude::*;
use crate::process::table::{get_all_processes, get_pgrp, get_process};

pub fn do_set_priority(which: PrioWhich, who: i32, nice: NiceValue) -> Result<()> {
    debug!(
        "set_priority: which: {:?}, who: {}, nice: {:?}",
        which, who, nice
    );

    let processes = get_processes(which, who)?;
    for process in processes.iter() {
        for thread in process.threads().iter() {
            *thread.nice().write().unwrap() = nice;
        }
    }
    Ok(())
}

pub fn do_get_priority(which: PrioWhich, who: i32) -> Result<NiceValue> {
    debug!("get_priority: which: {:?}, who: {}", which, who);

    let processes = get_processes(which, who)?;
    let nice = {
        let mut nice = NiceValue::MAX;
        for process in processes.iter() {
            let main_thread = process
                .main_thread()
                .ok_or_else(|| errno!(ESRCH, "invalid pid"))?;
            let current_nice = main_thread.nice().read().unwrap();
            // Returns the highest priority enjoyed by the processes
            if *current_nice < nice {
                nice = *current_nice;
            }
        }
        nice
    };
    Ok(nice)
}

fn get_processes(which: PrioWhich, who: i32) -> Result<Vec<crate::process::ProcessRef>> {
    let processes = match which {
        PrioWhich::PRIO_PROCESS => {
            let process = if who == 0 {
                current!().process().clone()
            } else {
                get_process(who as pid_t)?
            };
            vec![process]
        }
        PrioWhich::PRIO_PGRP => {
            let pgrp = if who == 0 {
                current!().process().pgrp()
            } else {
                get_pgrp(who as pid_t)?
            };
            pgrp.get_all_processes()
        }
        PrioWhich::PRIO_USER => {
            if who == 0 {
                get_all_processes()
            } else {
                warn!("only root user is supported in Occlum");
                return_errno!(ESRCH, "no such user");
            }
        }
    };
    if processes.is_empty() {
        return_errno!(ESRCH, "no such process");
    }

    Ok(processes)
}
