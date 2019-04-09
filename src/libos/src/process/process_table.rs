use super::*;
use std::sync::atomic::{AtomicU32, Ordering};

lazy_static! {
    static ref PROCESS_TABLE: SgxMutex<HashMap<pid_t, ProcessRef>> =
        { SgxMutex::new(HashMap::new()) };
}

pub fn put(pid: pid_t, process: ProcessRef) {
    PROCESS_TABLE.lock().unwrap().insert(pid, process);
}

pub fn remove(pid: pid_t) {
    PROCESS_TABLE.lock().unwrap().remove(&pid);
}

pub fn get(pid: pid_t) -> Result<ProcessRef, Error> {
    PROCESS_TABLE.lock().unwrap().get(&pid)
        .map(|pr| pr.clone())
        .ok_or_else(|| Error::new(Errno::ENOENT, "process not found"))
}

static NEXT_PID: AtomicU32 = AtomicU32::new(1);

pub fn alloc_pid() -> u32 {
    NEXT_PID.fetch_add(1, Ordering::SeqCst)
}

pub fn free_pid(pid: u32) {
    // PID 0 is reserved for idle thread, thus no need to free
    if pid == 0 {
        return;
    }
    // TODO:
}
