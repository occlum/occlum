use spin::rw_lock::RwLock;

use crate::executor::EXECUTOR;
use crate::prelude::*;
use crate::sched::Affinity;

/// A per-task scheduling-related info.
pub struct SchedInfo {
    last_thread_id: AtomicU32,
    affinity: RwLock<Affinity>,
}

impl SchedInfo {
    pub fn new() -> Self {
        static LAST_THREAD_ID: AtomicU32 = AtomicU32::new(0);

        let last_thread_id = {
            let last_thread_id =
                LAST_THREAD_ID.fetch_add(1, Ordering::Relaxed) % EXECUTOR.parallelism();
            AtomicU32::new(last_thread_id)
        };
        let affinity = RwLock::new(Affinity::new_full());

        Self {
            last_thread_id,
            affinity,
        }
    }

    pub fn last_thread_id(&self) -> u32 {
        self.last_thread_id.load(Ordering::Relaxed)
    }

    pub fn set_last_thread_id(&self, id: u32) {
        self.last_thread_id.store(id, Ordering::Relaxed);
    }

    pub fn affinity(&self) -> &RwLock<Affinity> {
        &self.affinity
    }
}
