use std::sync::atomic::{AtomicU32, Ordering::Relaxed};
use std::sync::Arc;

use crate::scheduler::local_scheduler::StatusNotifier;
use crate::scheduler::{SchedState, SchedTag};
use crate::util::AtomicBits;

/// vCPU selector.
///
/// A vCPU selector decides which vCPU a schedulable entity should run on.
/// vCPU selectors are designed to make _fast_ and _sensible_ vCPU
/// selection decisions. As such, decisions are made by following
/// a set of simple rules.
///
/// First and foremost, vCPU assignment must respect the
/// affinity mask of an entity. Beyond that, vCPU selectors adopt some heuristics
/// to make sensible decisions. For instance, cache-hot and blocking information
/// is adopted in selecting mechanism. The selector keeps blocking tasks as evenly
/// distributed on different vpus as possible and reduce their migration
///
/// For spawned tasks, the scheduler distributes them to different core in consecutive order.
/// For waked tasks, the scheduler apply different scheduling policies according to
/// task heuristical pattern.
///
/// Blocking task needs to utilize cpu exclusively. So they choose:
/// 1. Idle vCPUs (which are busy looping for available entities to schedule)
/// 2. Unparking sleep vCPU (unparking overhead is trivial to blocking task running time)
/// 3. Last vCPUs (hit cpu L1/L2 cache as possible)
///
/// Non-blocking task requires to respond as quickly.
/// 1. Idle vCPUs (which are busy looping for available entities to schedule);
/// 2. Active and light vCPUs (which are not sleeping and have no blocking task);
/// 3. Unparking sleep vCPU if no active and light vCPUs
/// 4. Last vCPUs (hit cpu L1/L2 cache as possible)
pub struct VcpuSelector {
    idle_vcpu_mask: AtomicBits,
    sleep_vcpu_mask: AtomicBits,
    heavy_vcpu_mask: AtomicBits,
    num_running_vcpus: AtomicU32,
    num_vcpus: u32,
}

impl StatusNotifier for Arc<VcpuSelector> {
    fn notify_idle_status(&self, vcpu: u32, is_idle: bool) {
        self.idle_vcpu_mask.set(vcpu as usize, is_idle);
    }

    fn notify_sleep_status(&self, vcpu: u32, is_sleep: bool) {
        self.sleep_vcpu_mask.set(vcpu as usize, is_sleep);
        if is_sleep {
            self.num_running_vcpus.fetch_sub(1, Relaxed);
        } else {
            self.num_running_vcpus.fetch_add(1, Relaxed);
        }
        // TODO: assert num_running_vcpus.
    }

    fn notify_heavy_status(&self, vcpu: u32, is_heavy: bool) {
        self.heavy_vcpu_mask.set(vcpu as usize, is_heavy);
    }
}

impl VcpuSelector {
    /// Create an instance.
    pub fn new(num_vcpus: u32) -> Self {
        Self {
            idle_vcpu_mask: AtomicBits::new_zeroes(num_vcpus as usize),
            sleep_vcpu_mask: AtomicBits::new_zeroes(num_vcpus as usize),
            heavy_vcpu_mask: AtomicBits::new_zeroes(num_vcpus as usize),
            num_running_vcpus: AtomicU32::new(num_vcpus),
            num_vcpus,
        }
    }

    #[inline(always)]
    pub fn sleep_vcpu_mask(&self) -> &AtomicBits {
        &self.sleep_vcpu_mask
    }

    #[inline(always)]
    pub fn num_running_vcpus(&self) -> u32 {
        self.num_running_vcpus.load(Relaxed)
    }

    /// Select the vCPU for an entity, given its state.
    ///
    /// If the current thread is used as a vCPU, then the vCPU number should
    /// be provided.
    pub fn select_vcpu(&self, sched_state: &SchedState, has_this_vcpu: Option<u32>) -> u32 {
        // Need to respect the CPU affinity mask
        let affinity = sched_state.affinity();
        debug_assert!(affinity.iter_ones().count() > 0);

        // Check whether this vCPU is in the affinity mask
        let has_this_vcpu = {
            if let Some(this_vcpu) = has_this_vcpu {
                if affinity.get(this_vcpu as usize) {
                    Some(this_vcpu)
                } else {
                    None
                }
            } else {
                None
            }
        };
        // Check whether the last vCPU is in the affinity mask
        let has_last_vcpu = {
            if let Some(last_vcpu) = sched_state.vcpu() {
                if affinity.get(last_vcpu as usize) {
                    Some(last_vcpu)
                } else {
                    None
                }
            } else {
                None
            }
        };

        // If the task is the first time to enqueue, use round-robin strategy to balance vCPU load
        if has_last_vcpu.is_none() {
            return self.select_vcpu_round_robin(sched_state);
        }

        // Todo: integrate the information of pending tasks into vCPU selecting strategy.
        // Consider the situation that this vCPU has large number of pending tasks,
        // but its queue length is zero and in the idle state.
        let sched_tag = sched_state.update_sched_tag();
        if sched_tag.contains(SchedTag::BLOCK) {
            self.select_vcpu_blocking(sched_state, has_last_vcpu.unwrap(), has_this_vcpu)
        } else {
            self.select_vcpu_nonblocking(sched_state, has_last_vcpu.unwrap(), has_this_vcpu)
        }
    }

    /// Select vcpu for blocking tasks
    #[inline]
    fn select_vcpu_round_robin(&self, sched_state: &SchedState) -> u32 {
        static NEXT_VCPU: AtomicU32 = AtomicU32::new(0);
        let affinity = sched_state.affinity();
        loop {
            let vcpu = NEXT_VCPU.fetch_add(1, Relaxed) % self.num_vcpus;
            if affinity.get(vcpu as usize) {
                return vcpu;
            }
        }
    }

    /// Select vcpu for blocking tasks
    #[inline]
    fn select_vcpu_blocking(
        &self,
        sched_state: &SchedState,
        last_vcpu: u32,
        _has_this_vcpu: Option<u32>,
    ) -> u32 {
        let idle_vcpu_mask = &self.idle_vcpu_mask;
        let sleep_vcpu_mask = &self.sleep_vcpu_mask;
        let affinity = sched_state.affinity();

        // Select last run vCPU if idle, for hitting cache
        if idle_vcpu_mask.get(last_vcpu as usize) {
            return last_vcpu;
        }

        // Select any idle vCPU for load balancing
        let has_idle_vcpu = idle_vcpu_mask
            .iter_ones()
            .find(|idle_vcpu| affinity.get(*idle_vcpu));
        if let Some(idle_vcpu) = has_idle_vcpu {
            return idle_vcpu as u32;
        }

        // Select sleep vCPU, blocking tasks need to utilize vCPU exclusively
        if sleep_vcpu_mask.get(last_vcpu as usize) {
            return last_vcpu;
        }
        let has_sleep_vcpu = sleep_vcpu_mask
            .iter_ones()
            .find(|sleep_vcpu| affinity.get(*sleep_vcpu));
        if let Some(sleep_vcpu) = has_sleep_vcpu {
            return sleep_vcpu as u32;
        }

        // Select last run vCPU if no sleep vCPU
        last_vcpu
    }

    /// Select vcpu for non-blocking tasks
    #[inline]
    fn select_vcpu_nonblocking(
        &self,
        sched_state: &SchedState,
        last_vcpu: u32,
        has_this_vcpu: Option<u32>,
    ) -> u32 {
        let affinity = sched_state.affinity();
        // Give preferance to idle vCPU in vCPU selecting strategy
        {
            let idle_vcpu_mask = &self.idle_vcpu_mask;

            // Select the last vCPU that the entity runs on, if it is idle.
            // Prefer last vCPU to avoid switching real cpu for one task.
            if idle_vcpu_mask.get(last_vcpu as usize) {
                return last_vcpu;
            }

            // Select this vCPU, if it is idle.
            if let Some(this_vcpu) = has_this_vcpu {
                if idle_vcpu_mask.get(this_vcpu as usize) {
                    return this_vcpu;
                }
            }

            // Select any idle vCPU.
            let has_idle_vcpu = idle_vcpu_mask
                .iter_ones()
                .find(|idle_vcpu| affinity.get(*idle_vcpu));
            if let Some(idle_vcpu) = has_idle_vcpu {
                return idle_vcpu as u32;
            }
        }

        let sleep_vcpu_mask = &self.sleep_vcpu_mask;
        let heavy_vcpu_mask = &self.heavy_vcpu_mask;

        // If no idle vCPU, select active vCPU and avoid waking up sleep vCPU.
        // Since waking up sleep vCPU need to unpark thread, which increase performance overhead.
        // Besides, if there are large amounts of idle vCPUs but a small number of runnable threads,
        // those threads are prone to switch run between different vCPU,
        // which also significantly increase performance overhead.
        {
            // Select the last vCPU that the entity runs on, if it is active.
            if !sleep_vcpu_mask.get(last_vcpu as usize) {
                return last_vcpu;
            }

            let has_active_light_vcpu = sleep_vcpu_mask.iter_zeroes().find(|active_light_vcpu| {
                affinity.get(*active_light_vcpu)
                    && !heavy_vcpu_mask.get(*active_light_vcpu as usize)
            });
            if let Some(active_light_vcpu) = has_active_light_vcpu {
                return active_light_vcpu as u32;
            }
        }

        // If no active and light vCPU, waking up any sleep vCPU.
        {
            if sleep_vcpu_mask.get(last_vcpu as usize) {
                return last_vcpu;
            }

            let has_sleep_vcpu = sleep_vcpu_mask
                .iter_ones()
                .find(|sleep_vcpu| affinity.get(*sleep_vcpu));
            if let Some(sleep_vcpu) = has_sleep_vcpu {
                return sleep_vcpu as u32;
            }
        }

        // The last vCPU that the entity runs on, regardless of whether it is
        // active or not (as long as it is in the affinity mask)
        last_vcpu
    }
}
