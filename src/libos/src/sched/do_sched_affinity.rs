use super::cpu_set::CpuSet;
use crate::prelude::*;
use crate::process::ThreadRef;

pub fn do_sched_getaffinity(tid: pid_t) -> Result<CpuSet> {
    debug!("do_sched_getaffinity tid: {}", tid);
    let thread = get_thread_by_tid(tid)?;
    let sched = thread.sched().lock().unwrap();
    let affinity = sched.affinity().clone();
    Ok(affinity)
}

pub fn do_sched_setaffinity(tid: pid_t, new_affinity: CpuSet) -> Result<()> {
    debug!(
        "do_sched_setaffinity tid: {}, new_affinity = {:?}",
        tid, &new_affinity
    );
    let thread = get_thread_by_tid(tid)?;
    let mut sched = thread.sched().lock().unwrap();
    sched.set_affinity(new_affinity)?;
    Ok(())
}

fn get_thread_by_tid(tid: pid_t) -> Result<ThreadRef> {
    if tid == 0 {
        Ok(current!())
    } else {
        crate::process::table::get_thread(tid)
    }
}
