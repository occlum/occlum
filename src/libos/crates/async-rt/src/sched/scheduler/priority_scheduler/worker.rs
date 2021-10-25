use super::queue::PeekableTaskQueue;
use super::Injector;
use crate::prelude::*;
use crate::sched::{SchedPriority, MAX_QUEUED_TASKS};
use crate::task::Task;

pub(crate) struct Worker {
    high_pri_queue: PeekableTaskQueue,
    normal_pri_queue: PeekableTaskQueue,
    low_pri_queue: PeekableTaskQueue,
    #[cfg(feature = "use_latency")]
    latency: AtomicU64,
    // pri_number is used to decide priority when pop
    // since we use non-strict priority scheduling.
    pri_number: AtomicU8,
}

impl Worker {
    pub fn new() -> Self {
        Self {
            high_pri_queue: PeekableTaskQueue::new(Some(MAX_QUEUED_TASKS)),
            normal_pri_queue: PeekableTaskQueue::new(Some(MAX_QUEUED_TASKS)),
            low_pri_queue: PeekableTaskQueue::new(Some(MAX_QUEUED_TASKS)),
            #[cfg(feature = "use_latency")]
            latency: AtomicU64::new(0),
            pri_number: AtomicU8::new(0),
        }
    }

    pub fn push(&self, task: Arc<Task>, injector: &Injector) -> bool {
        if let Err(t) = self.queue(task.sched_info().priority()).push(task) {
            injector.push(t);
            return false;
        }
        true
    }

    pub fn pop(&self) -> Option<Arc<Task>> {
        // TODO: Use a better method to decide priority
        // e.g., specify probabilities and generate a random.
        let pri_number = self.inc_pri_number() % 8;
        match pri_number {
            0..=4 => {
                if let Some(task) = self.high_pri_queue.pop() {
                    return Some(task);
                }
                if let Some(task) = self.normal_pri_queue.pop() {
                    return Some(task);
                }
                if let Some(task) = self.low_pri_queue.pop() {
                    return Some(task);
                }
            }
            5 | 6 => {
                if let Some(task) = self.normal_pri_queue.pop() {
                    return Some(task);
                }
                if let Some(task) = self.high_pri_queue.pop() {
                    return Some(task);
                }
                if let Some(task) = self.low_pri_queue.pop() {
                    return Some(task);
                }
            }
            _ => {
                if let Some(task) = self.low_pri_queue.pop() {
                    return Some(task);
                }
                if let Some(task) = self.high_pri_queue.pop() {
                    return Some(task);
                }
                if let Some(task) = self.normal_pri_queue.pop() {
                    return Some(task);
                }
            }
        }
        None
    }

    pub fn pop_with_priority(&self, priority: SchedPriority) -> Option<Arc<Task>> {
        self.queue(priority).pop()
    }

    /// Pop the front task from specific queue if the front task passed the check function.
    ///
    /// Return the result of check function and the front task if check function returns `Some`.
    /// Return `None` if check function returns `None`.
    pub fn pop_with_priority_if_pass_check<T, F>(
        &self,
        f: F,
        priority: SchedPriority,
    ) -> Option<(T, Arc<Task>)>
    where
        F: FnOnce(&Arc<Task>) -> Option<T>,
    {
        self.queue(priority).pop_if_pass_check(f)
    }

    pub fn len(&self, priority: SchedPriority) -> usize {
        self.queue(priority).len()
    }

    pub fn is_empty(&self, priority: SchedPriority) -> bool {
        self.queue(priority).is_empty()
    }

    pub fn capacity(&self, _priority: SchedPriority) -> usize {
        MAX_QUEUED_TASKS
    }

    pub fn is_full(&self, priority: SchedPriority) -> bool {
        self.queue(priority).len() == MAX_QUEUED_TASKS
    }

    #[cfg(feature = "use_latency")]
    pub fn latency(&self) -> u64 {
        self.latency.load(Ordering::Relaxed)
    }

    #[cfg(feature = "use_latency")]
    pub fn update_latency(&self, latency: u64) {
        static ALPHA: f64 = 0.5;
        self.latency
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |old| {
                Some(old / 2 + latency / 2)
            })
            .unwrap();
    }

    fn inc_pri_number(&self) -> u8 {
        self.pri_number.fetch_add(1, Ordering::Relaxed)
    }

    fn queue(&self, priority: SchedPriority) -> &PeekableTaskQueue {
        match priority {
            SchedPriority::High => &self.high_pri_queue,
            SchedPriority::Normal => &self.normal_pri_queue,
            SchedPriority::Low => &self.low_pri_queue,
        }
    }
}
