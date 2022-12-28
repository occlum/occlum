//! Scheduler.

use crate::prelude::*;
use crate::vcpu;
use std::sync::Arc;

mod entity;
mod local_scheduler;
mod priority;
mod timeslice;
mod vcpu_selector;
mod yield_;

pub use self::yield_::yield_now;
pub use entity::{SchedEntity, SchedState};
pub use local_scheduler::{LocalScheduler, LocalSchedulerGuard, StatusNotifier};
pub use priority::Priority;
use vcpu_selector::VcpuSelector;

/// A scheduler for scheduling tasks on a fixed number of vCPUs.
///
/// * Fairness. All tasks are assigned with fair portion of the CPU time.
/// * Efficiency. O(1) complexity for enqueueing and dequeueing tasks.
/// * Interactivity. I/O-bound task are considered more "interactive" than
/// CPU-bound tasks, thus getting priority boost.
pub struct Scheduler<E> {
    pub local_schedulers: Arc<Box<[LocalScheduler<E>]>>,
    vcpu_selector: Arc<VcpuSelector>,
    num_tasks: AtomicU32,
}

impl<E: SchedEntity> Scheduler<E> {
    /// Create an instance of the given number of vcpus.
    pub fn new(num_vcpus: u32) -> Self {
        debug_assert!(num_vcpus > 0);
        let vcpu_selector = Arc::new(VcpuSelector::new(num_vcpus));

        let local_schedulers = Arc::new(
            (0..num_vcpus)
                .map(|this_vcpu| {
                    let status_notifier = vcpu_selector.clone();
                    LocalScheduler::new(this_vcpu, status_notifier)
                })
                .collect::<Vec<LocalScheduler<_>>>()
                .into_boxed_slice(),
        );

        let num_tasks = AtomicU32::new(0);

        Self {
            local_schedulers,
            vcpu_selector,
            num_tasks,
        }
    }

    /// Enqueue a scheduable entity.
    ///
    /// If the current thread serves a vCPU, its vCPU ID should also
    /// be provided so that the scheduler can make more informed
    /// decisions as to which vCPU should be select to execute the vCPU.
    ///
    /// If the current thread is not a vCPU, then it is still ok to
    /// enqueue entities. Just leave `this_vcpu` as `None`.
    pub fn enqueue(&self, entity: &Arc<E>) {
        let this_vcpu = vcpu::get_current();
        let target_vcpu = self
            .vcpu_selector
            .select_vcpu(entity.sched_state(), this_vcpu);
        let local_scheduler = &self.local_schedulers[target_vcpu as usize];
        local_scheduler.enqueue(entity);

        self.num_tasks.fetch_add(1, Ordering::Relaxed);
        self.wake_vcpus();
    }

    /// Dequeue a scheduable entity on the current vCPU.
    pub fn dequeue(&self) -> Option<Arc<E>> {
        let this_vcpu = vcpu::get_current().unwrap();
        let local_scheduler = &self.local_schedulers[this_vcpu as usize];
        let local_guard = local_scheduler.lock();
        let task = local_guard.dequeue();
        if task.is_some() {
            self.num_tasks.fetch_sub(1, Ordering::Relaxed);
        }
        task
    }

    /// Get the number of vCPUs.
    pub fn num_vcpus(&self) -> u32 {
        self.local_schedulers.len() as u32
    }

    #[inline(always)]
    fn wake_vcpus(&self) {
        let num_tasks = self.num_tasks.load(Ordering::Relaxed);
        let num_running_vcpus = self.vcpu_selector.num_running_vcpus();

        // Determine how many sleep vcpus need to be waked:
        // We choose the ratio between active vcpus and tasks in queue is 1 / 1.5,
        // because unpark is an operation with large performance loss.
        // We want to balance the performance loss between park/unpark switching and
        // lack of active vcpus. If the ratio between vcpus and tasks in queue equals to 1,
        // the large amounts of unparking operation would cause the average latency up to 2 times.
        if num_tasks * 2 / 3 > num_running_vcpus {
            let num_wake = (self.num_vcpus() - num_running_vcpus)
                .min(num_tasks * 2 / 3 - num_running_vcpus) as usize;
            self.vcpu_selector
                .sleep_vcpu_mask()
                .iter_ones()
                .take(num_wake)
                .for_each(|vcpu_idx| vcpu::unpark(vcpu_idx));
        }
    }
}
