use spin::rw_lock::RwLock;

use crate::executor::EXECUTOR;
use crate::prelude::*;
use crate::sched::Affinity;

/// A per-task scheduling-related info.
pub struct SchedInfo {
    last_thread_id: AtomicU32,
    affinity: RwLock<Affinity>,
    priority: RwLock<SchedPriority>,
}

impl SchedInfo {
    pub fn new(priority: SchedPriority) -> Self {
        static LAST_THREAD_ID: AtomicU32 = AtomicU32::new(0);

        let last_thread_id = {
            let last_thread_id =
                LAST_THREAD_ID.fetch_add(1, Ordering::Relaxed) % EXECUTOR.parallelism();
            AtomicU32::new(last_thread_id)
        };
        let affinity = RwLock::new(Affinity::new_full());
        let priority = RwLock::new(priority);

        Self {
            last_thread_id,
            affinity,
            priority,
        }
    }

    pub fn affinity(&self) -> &RwLock<Affinity> {
        &self.affinity
    }

    pub fn priority(&self) -> SchedPriority {
        *self.priority.read()
    }

    pub fn set_priority(&self, priority: SchedPriority) {
        *self.priority.write() = priority;
    }

    pub(crate) fn last_thread_id(&self) -> u32 {
        self.last_thread_id.load(Ordering::Relaxed)
    }

    pub(crate) fn set_last_thread_id(&self, id: u32) {
        self.last_thread_id.store(id, Ordering::Relaxed);
    }

    pub fn affinity(&self) -> &RwLock<Affinity> {
        &self.affinity
    }

#[derive(Debug, Clone, Copy)]
pub enum SchedPriority {
    High,
    Normal,
    Low,
}
