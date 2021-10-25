use spin::rw_lock::RwLock;

use crate::executor::EXECUTOR;
use crate::prelude::*;
use crate::sched::Affinity;

/// A per-task scheduling-related info.
pub struct SchedInfo {
    last_thread_id: AtomicU32,
    affinity: RwLock<Affinity>,
    priority: RwLock<SchedPriority>,
    #[cfg(feature = "use_latency")]
    enqueue_epochs: AtomicU64,
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
        #[cfg(feature = "use_latency")]
        let enqueue_epochs = AtomicU64::new(0);

        Self {
            last_thread_id,
            affinity,
            priority,
            #[cfg(feature = "use_latency")]
            enqueue_epochs,
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

    #[cfg(feature = "use_latency")]
    pub(crate) fn enqueue_epochs(&self) -> u64 {
        self.enqueue_epochs.load(Ordering::Relaxed)
    }

    #[cfg(feature = "use_latency")]
    pub(crate) fn set_enqueue_epochs(&self, data: u64) {
        self.enqueue_epochs.store(data, Ordering::Relaxed);
    }
}

#[derive(Debug, Clone, Copy)]
pub enum SchedPriority {
    High,
    Normal,
    Low,
}
