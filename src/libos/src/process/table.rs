use super::{ProcessGrpRef, ProcessRef, ThreadRef};
use crate::prelude::*;

pub fn get_pgrp(pgid: pid_t) -> Result<ProcessGrpRef> {
    PROCESSGRP_TABLE.lock().unwrap().get(pgid)
}

pub(super) fn add_pgrp(pgrp: ProcessGrpRef) -> Result<()> {
    PROCESSGRP_TABLE.lock().unwrap().add(pgrp.pgid(), pgrp)
}

pub(super) fn del_pgrp(pgid: pid_t) -> Result<ProcessGrpRef> {
    PROCESSGRP_TABLE.lock().unwrap().del(pgid)
}

pub fn get_pgrp_number(pgid: pid_t) -> usize {
    PROCESSGRP_TABLE.lock().unwrap().len()
}

pub fn get_all_pgrp() -> Vec<ProcessGrpRef> {
    PROCESSGRP_TABLE
        .lock()
        .unwrap()
        .iter()
        .map(|(_, pgrp_ref)| pgrp_ref.clone())
        .collect()
}

pub fn get_process(pid: pid_t) -> Result<ProcessRef> {
    PROCESS_TABLE.lock().unwrap().get(pid)
}

pub fn get_all_processes() -> Vec<ProcessRef> {
    PROCESS_TABLE
        .lock()
        .unwrap()
        .iter()
        .map(|(_, proc_ref)| proc_ref.clone())
        .collect()
}

pub fn get_all_threads() -> Vec<ThreadRef> {
    THREAD_TABLE
        .lock()
        .unwrap()
        .iter()
        .map(|(_, proc_ref)| proc_ref.clone())
        .collect()
}

pub(super) fn add_process(process: ProcessRef) -> Result<()> {
    PROCESS_TABLE.lock().unwrap().add(process.pid(), process)
}

pub(super) fn del_process(pid: pid_t) -> Result<ProcessRef> {
    let mut process_table = PROCESS_TABLE.lock().unwrap();
    let res = process_table.del(pid);

    // update the clean flag to true, and notify the waiting thread
    if process_table.iter().len() == 0 {
        let (lock, cvar) = &*PROCESSES_STATUS.clone();
        let mut clean = lock.lock().unwrap();
        *clean = true;
        // We notify the condvar that the value has changed.
        cvar.notify_one();
    }
    res
}

pub fn wait_all_process_exit() {
    let (lock, cvar) = &*PROCESSES_STATUS.clone();

    // set the clean flag to false before check the existing process number
    let mut clean = lock.lock().unwrap();
    *clean = false;
    // must drop the lock before check the existing process number
    drop(clean);

    if PROCESS_TABLE.lock().unwrap().iter().len() == 0 {
        return;
    }

    let mut clean = lock.lock().unwrap();
    // As long as the value inside the flag is `false`, we wait.
    while !*clean {
        clean = cvar.wait(clean).unwrap();
    }
}

pub fn replace_process(pid: pid_t, new_process: ProcessRef) -> Result<()> {
    del_process(pid);
    add_process(new_process)
}

pub fn get_thread(tid: pid_t) -> Result<ThreadRef> {
    THREAD_TABLE.lock().unwrap().get(tid)
}

pub(super) fn add_thread(thread: ThreadRef) -> Result<()> {
    THREAD_TABLE.lock().unwrap().add(thread.tid(), thread)
}

pub(super) fn del_thread(tid: pid_t) -> Result<ThreadRef> {
    THREAD_TABLE.lock().unwrap().del(tid)
}

pub(super) fn replace_thread(tid: pid_t, new_thread: ThreadRef) -> Result<()> {
    del_thread(tid);
    add_thread(new_thread)
}

pub fn debug() {
    println!("process table = {:#?}", PROCESS_TABLE.lock().unwrap());
    println!("thread table = {:#?}", THREAD_TABLE.lock().unwrap());
    //println!("idle = {:#?}", *super::IDLE);
}

lazy_static! {
    static ref PROCESS_TABLE: SgxMutex<Table<ProcessRef>> =
        { SgxMutex::new(Table::<ProcessRef>::with_capacity(8)) };
    static ref THREAD_TABLE: SgxMutex<Table<ThreadRef>> =
        { SgxMutex::new(Table::<ThreadRef>::with_capacity(8)) };
    static ref PROCESSGRP_TABLE: SgxMutex<Table<ProcessGrpRef>> =
        { SgxMutex::new(Table::<ProcessGrpRef>::with_capacity(4)) };
    static ref PROCESSES_STATUS: Arc<(SgxMutex<bool>, SgxCondvar)> =
        Arc::new((SgxMutex::new(false), SgxCondvar::new()));
}

#[derive(Debug, Clone)]
struct Table<I: Debug + Clone + Send + Sync> {
    map: HashMap<pid_t, I>,
}

impl<I: Debug + Clone + Send + Sync> Table<I> {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            map: HashMap::with_capacity(capacity),
        }
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn iter(&self) -> std::collections::hash_map::Iter<'_, pid_t, I> {
        self.map.iter()
    }

    pub fn get(&self, id: pid_t) -> Result<I> {
        self.map
            .get(&id)
            .map(|item_ref| item_ref.clone())
            .ok_or_else(|| errno!(ESRCH, "id does not exist"))
    }

    pub fn add(&mut self, id: pid_t, item: I) -> Result<()> {
        if self.map.contains_key(&id) {
            return_errno!(EEXIST, "id is already added");
        }
        self.map.insert(id, item);
        Ok(())
    }

    pub fn del(&mut self, id: pid_t) -> Result<I> {
        if !self.map.contains_key(&id) {
            return_errno!(ENOENT, "id does not exist");
        }
        Ok(self.map.remove(&id).unwrap())
    }
}
