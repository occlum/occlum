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

        Self {
            local_schedulers,
            vcpu_selector,
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
    }

    /// Dequeue a scheduable entity on the current vCPU.
    pub fn dequeue(&self) -> Option<Arc<E>> {
        let this_vcpu = vcpu::get_current().unwrap();
        let local_scheduler = &self.local_schedulers[this_vcpu as usize];
        let local_guard = local_scheduler.lock();
        local_guard.dequeue()
    }

    /// Get the number of vCPUs.
    pub fn num_vcpus(&self) -> u32 {
        self.local_schedulers.len() as u32
    }
}
