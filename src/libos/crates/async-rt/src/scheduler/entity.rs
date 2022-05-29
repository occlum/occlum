use std::sync::atomic::{AtomicBool, AtomicI8, AtomicU32, Ordering::*};

use super::timeslice::calculate_timeslice;
use crate::scheduler::Priority;
use crate::util::AtomicBits;

/// A schedulable entity.
pub trait SchedEntity {
    /// Returns the state of the schedulable entity. The scheduler maintains
    /// scheduling-related information in the state.
    fn sched_state(&self) -> &SchedState;
}

/// The scheduler-related state of a schedulable entity.
///
/// The user of a scheduler, e.g., an executor, provide entity-specific inputs
/// to the scheduler via `SchedState`. When creating a schedulable entity,
/// the user attaches with it an instance of `SchedState`.
#[derive(Debug)]
pub struct SchedState {
    base_prio: Priority,
    prio_adjust: AtomicI8,
    is_enqueued: AtomicBool,
    timeslice_ms: AtomicU32,
    affinity: AtomicBits,
    vcpu: AtomicU32,
}

impl SchedState {
    /// Create a new instance given the base priority.
    pub fn new(num_vcpus: u32, base_prio: Priority) -> Self {
        let new_self = Self {
            base_prio,
            prio_adjust: AtomicI8::new(0),
            is_enqueued: AtomicBool::new(false),
            timeslice_ms: AtomicU32::new(0),
            affinity: AtomicBits::new_ones(num_vcpus as usize),
            vcpu: AtomicU32::new(Self::NONE_VCPU),
        };
        new_self.assign_timeslice();
        new_self
    }

    /// Returns the base priority.
    ///
    /// The base priority has a fixed value given by user.
    /// The base priority affects the lengths of the timeslices that
    /// an entity is assigned by the scheduler.
    pub fn base_prio(&self) -> Priority {
        self.base_prio
    }

    /// Returns the effective priority.
    ///
    /// The effective priority is determined by the scheduling algorithm.
    /// It reflects how "interactive" an entity is from the perspective of
    /// the scheduler. I/O-bound code is more interactive, while
    /// CPU-bound code is less interactive.
    /// The scheduler does its best to prioritize interactive schedulable
    /// entities to minimize their I/O latencies.
    ///
    /// The scheduler needs users inputs to decide how interactive an entity is.
    /// To do so, the user should call `report_sleep`, `report_preemption`,
    /// and `report_yield` methods to report some remarkable behaviors of
    /// an entity.
    pub fn effective_prio(&self) -> Priority {
        self.base_prio + self.prio_adjust.load(Relaxed)
    }

    // Two parameters to constraint the impact of priority adjustments.
    // The given values seems to be some sensible, heuristic values.
    const MAX_PRIO_ADJUST: i8 = 8;
    const MIN_PRIO_ADJUST: i8 = -8;

    /// Returns the affinity mask.
    pub fn affinity(&self) -> &AtomicBits {
        &self.affinity
    }

    /// Get the last vCPU that an entity runs on.
    ///
    /// `None` is returned if the entity is new and hasn't run yet.
    pub fn vcpu(&self) -> Option<u32> {
        let vcpu = self.vcpu.load(Relaxed);
        if vcpu != Self::NONE_VCPU {
            Some(vcpu)
        } else {
            None
        }
    }

    /// Set the vCPU that an entity runs on.
    pub(super) fn set_vcpu(&self, vcpu: u32) {
        self.vcpu.store(vcpu, Relaxed)
    }

    const NONE_VCPU: u32 = u32::max_value();

    /// Report that the associated schedulable entity slept. Sleep
    /// increases the effective priority of the entity.
    pub fn report_sleep(&self) {
        let prio_adjust = self.prio_adjust.load(Relaxed);
        if prio_adjust >= Self::MAX_PRIO_ADJUST {
            return;
        }
        self.prio_adjust.store(prio_adjust + 1, Relaxed);
    }

    /// Report that the associated schedulable entity is preempted. Preemption
    /// decreases the effective priority of the entity.
    pub fn report_preemption(&self) {
        let prio_adjust = self.prio_adjust.load(Relaxed);
        if prio_adjust <= Self::MIN_PRIO_ADJUST {
            return;
        }
        self.prio_adjust.store(prio_adjust - 1, Relaxed);
    }

    /// Report that the associated schedulable entity yielded. Yield
    /// decreases the effective priority of the entity.
    pub fn report_yield(&self) {
        let prio_adjust = self.prio_adjust.load(Relaxed);
        // We do not make adjustment negative due to yield. Going negative seems
        // to be an unfair punishment to entities that are willing to give CPU time
        // cooperatively.
        if prio_adjust <= 0 {
            return;
        }
        self.prio_adjust.store(prio_adjust - 1, Relaxed);
    }

    /// Report that some time (in ms) has elapsed, consuming the
    /// assigned timeslice and returning the remaining timeslice (in ms).
    pub fn elapse(&self, elapsed_ms: u32) -> u32 {
        let mut remain_ms = self.timeslice_ms.load(Relaxed);
        if remain_ms > elapsed_ms {
            remain_ms -= elapsed_ms;
        } else {
            remain_ms = 0;
        }
        self.timeslice_ms.store(remain_ms, Relaxed);
        remain_ms
    }

    /// Returns the remaining timeslice in ms.
    pub fn timeslice(&self) -> u32 {
        self.timeslice_ms.load(Relaxed)
    }

    /// Assign a new timeslice. Used internally by the scheduler.
    pub(crate) fn assign_timeslice(&self) {
        let new_timeslice_ms = calculate_timeslice(self);
        self.timeslice_ms.store(new_timeslice_ms, Relaxed)
    }

    /// Set is_enqueued to true, returning the old value.
    ///
    /// The is_enqueued state helps the scheduler to avoid
    /// enqueueing an entity multiple times in a single
    /// scheduler or even in different schedulers.
    pub(crate) fn set_enqueued(&self) -> bool {
        self.is_enqueued.swap(true, Acquire)
    }

    /// Get is_enqueued
    pub(crate) fn is_enqueued(&self) -> bool {
        self.is_enqueued.load(Relaxed)
    }

    /// Set is_enqueued to false.
    pub(crate) fn clear_enqueued(&self) {
        self.is_enqueued.store(false, Release);
    }
}
