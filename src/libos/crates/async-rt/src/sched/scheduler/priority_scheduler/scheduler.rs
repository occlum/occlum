//! PriorityScheduler
//!
//! In the priority scheduler, we divide three priorities: high, normal and low.
//! And normal priority is used by default.
//!
//! We have worker and injector. Worker is the local queue and injector is the
//! global queue. Each worker corresponds to a thread and has three priority queues.
//! The length of each queue is the same.
//!
//! We use non-strict priority scheduling. High probability gives priority to
//! selecting tasks from high priority, medium probability gives priority to
//! selecting tasks from normal priority, and low probability gives priority to
//! selecting tasks from low priority.
//!
//! When a worker has no task, it will sleep; If a task is assigned to this worker,
//! the worker will be waken up. When inserting a task into the scheduler, we will
//! select the most appropriate worker according to the affinity of the task, the
//! queue length and the latency of each worker. If the worker's local queue is full,
//! the new task is inserted into the injector.
//!
//! A budget will be set for each task. When there is remained budget, the last
//! scheduled worker will be used first for scheduling; When the budget runs out,
//! we will re-select the optimal worker and reset the budget.
//!
//! We will perform load balancing periodically. We will perform balancing for the
//! three priorities respectively. In load balancing, we will first check the injector
//! and move the tasks in the injector to the workers with fewer tasks; Then we will
//! balance the queue length of all workers and move the tasks of workers with more
//! tasks to workers with fewer tasks, so as to ensure that the queue length of all
//! workers is similar as far as possible.

use super::{Injector, Worker};
use crate::parks::Parks;
use crate::prelude::*;
use crate::sched::{SchedPriority, Scheduler, MAX_QUEUED_TASKS};
use crate::task::Task;
use spin::mutex::MutexGuard;

pub struct PriorityScheduler {
    parallelism: usize,
    workers: Vec<Worker>,
    injector: Injector,
    epochs: AtomicU64,
    rebalance_lock: Mutex<()>,
    rebalance_interval: u64,
    parks: Arc<Parks>,
}

impl PriorityScheduler {
    const REBALANCE_BASE_INTERVAL: u64 = 128;

    pub fn new(parks: Arc<Parks>) -> Self {
        let parallelism = parks.len();
        let workers = (0..parallelism).map(|_| Worker::new()).collect();
        let injector = Injector::new();
        let epochs = AtomicU64::new(0);
        let rebalance_lock = Mutex::new(());
        let rebalance_interval = Self::REBALANCE_BASE_INTERVAL * parallelism as u64;
        Self {
            parallelism,
            workers,
            injector,
            epochs,
            rebalance_lock,
            rebalance_interval,
            parks,
        }
    }

    /// Pick a thread for the task.
    /// Use the task's priority if not specify priority in args.
    fn pick_thread_for(&self, task: &Arc<Task>) -> usize {
        let last_thread_id = task.sched_info().last_thread_id() as usize;
        let priority = task.sched_info().priority();

        // Meaningfull sched data of candidates thread: (index, length)
        let mut candidates: Vec<(usize, u64)> = Vec::new();

        let affinity = task.sched_info().affinity().read();
        affinity.iter_ones().for_each(|idx| {
            candidates.push((idx, 0));
        });
        drop(affinity);

        for idx in 0..candidates.len() {
            candidates[idx].1 = self.workers[idx].len(priority) as u64;
        }

        let thread_id = self.pick_best_candidates(&candidates, last_thread_id);
        task.sched_info().set_last_thread_id(thread_id as u32);
        task.reset_budget();
        thread_id
    }

    /// Insert the task to specific worker. If the queue of worker is full, insert the task to injector.
    /// Return True if the task is inserted to worker, return False if the task is inserted to injector.
    fn insert_task(&self, task: Arc<Task>, thread_id: usize) -> bool {
        #[cfg(feature = "use_latency")]
        task.sched_info().set_enqueue_epochs(self.current_epochs());
        let insert_to_worker = self.workers[thread_id].push(task, &self.injector);
        if insert_to_worker {
            self.parks.unpark(thread_id);
        }
        insert_to_worker
    }

    fn pick_best_candidates(
        &self,
        // candidates: (index, length)
        candidates: &Vec<(usize, u64)>,
        last_thread_id: usize,
    ) -> usize {
        assert!(candidates.len() != 0);

        let min_len_candidate = candidates.iter().min_by_key(|x| x.1).unwrap();
        if min_len_candidate.0 == last_thread_id {
            return min_len_candidate.0;
        }

        for candidate in candidates {
            if candidate.0 == last_thread_id {
                // TODO: Find a better difference
                if candidate.1 - min_len_candidate.1 <= 2 {
                    return candidate.0;
                }
                return min_len_candidate.0;
            }
        }

        min_len_candidate.0
    }


    fn try_rebalance_workload(&self) {
        if let Some(guard) = self.rebalance_lock.try_lock() {
            self.do_rebalance_for(&guard, SchedPriority::High);
            self.do_rebalance_for(&guard, SchedPriority::Normal);
            self.do_rebalance_for(&guard, SchedPriority::Low);
        }
    }

    fn do_rebalance_for(&self, guard: &MutexGuard<()>, priority: SchedPriority) {
        self.steal_tasks_from_injector(guard, priority);
        self.steal_tasks_from_workers(guard, priority);
    }

    fn steal_tasks_from_injector(&self, _guard: &MutexGuard<()>, priority: SchedPriority) {
        let mut worker_lens: Vec<usize> = self.workers.iter().map(|w| w.len(priority)).collect();

        while let Some(task) = self.injector.pop_with_priority(priority) {
            let affinity = task.sched_info().affinity().read();
            // Find the thread with the shortest queue.
            let idx = affinity.get_best_thread_by_length(&worker_lens);
            drop(affinity);

            if !self.insert_task(task, idx) {
                // This worker's queue is full. We think workers all have heavy workloads,
                // stop stealing tasks from injector to workers.
                break;
            }

            // Update worker queue length
            worker_lens[idx] += 1;
        }
    }

    fn steal_tasks_from_workers(&self, _guard: &MutexGuard<()>, priority: SchedPriority) {
        let mut worker_lens: Vec<usize> = self.workers.iter().map(|w| w.len(priority)).collect();

        let avg_len = worker_lens.iter().sum::<usize>() / self.parallelism;
        let heavy_limit = MAX_QUEUED_TASKS / 10 * 8 as usize;
        let target_len = core::cmp::max(core::cmp::min(avg_len, heavy_limit), 1);

        // Get the original index of sorted workers lengths.
        let sorted_idx: Vec<usize> = {
            let mut sorted_lens: Vec<(usize, usize)> = worker_lens
                .iter()
                .enumerate()
                .map(|(idx, len)| (idx, *len))
                .collect();
            sorted_lens.sort_unstable_by_key(|v| v.1);
            sorted_lens.iter().map(|v| v.0).collect()
        };
        let (mut left, mut right) = (0, self.parallelism - 1);

        // Try to steal tasks from heavy workers (right side) to light workers (left side).
        while left < right {
            let src_idx = sorted_idx[right];
            if worker_lens[src_idx] <= target_len {
                // The worker is no longer heavy. Try next.
                right -= 1;
                continue;
            }

            let dst_idx = sorted_idx[left];
            if worker_lens[dst_idx] >= target_len {
                // The worker is no longer light. Try next.
                left += 1;
                continue;
            }

            let check_func = |taskref: &Arc<Task>| {
                let affinity = taskref.sched_info().affinity().read();
                if affinity.is_full() {
                    return Some(dst_idx);
                }
                let target_idx = affinity.get_best_thread_by_length(&worker_lens);
                drop(affinity);
                if worker_lens[target_idx] < target_len {
                    Some(target_idx)
                } else {
                    None
                }
            };

            // Try to get a task that meets the check function.
            if let Some((target_idx, task)) =
                self.workers[src_idx].pop_with_priority_if_pass_check(check_func, priority)
            {
                // Try to insert task to the worker.
                if self.insert_task(task, target_idx) {
                    // The insertion succeeds, update the length.
                    worker_lens[target_idx] += 1;
                } else {
                    // The worker's queue is full. Update the length.
                    worker_lens[target_idx] = MAX_QUEUED_TASKS;
                }
                worker_lens[src_idx] -= 1;
            } else {
                // Didn't get a qualified task, try next worker.
                right -= 1;
            }
        }
    }

    fn current_epochs(&self) -> u64 {
        self.epochs.load(Ordering::Relaxed)
    }

    fn inc_epochs(&self) -> u64 {
        self.epochs.fetch_add(1, Ordering::Relaxed)
    }
}

impl Scheduler for PriorityScheduler {
    fn enqueue_task(&self, task: Arc<Task>) {
        let thread_id = if task.has_remained_budget() {
            let last_thread_id = task.sched_info().last_thread_id() as usize;
            let use_last_thread_id = {
                let affinity = task.sched_info().affinity().read();
                affinity.get(last_thread_id)
            };
            if use_last_thread_id {
                // Fast path: just use last_thread_id.
                last_thread_id
            } else {
                // Slow path: the affinity is changed, need pick new thread.
                self.pick_thread_for(&task)
            }
        } else {
            // Slow path: the budget has run out, pick new thread.
            self.pick_thread_for(&task)
        };

        self.insert_task(task, thread_id);
    }

    fn dequeue_task(&self, thread_id: usize) -> Option<Arc<Task>> {
        // Increase epochs in each scheduling.
        let cnt = self.inc_epochs();
        // Try to do rebalance every once in a while.
        if cnt % self.rebalance_interval == 0 {
            self.try_rebalance_workload();
        }

        // Try to get a task from the worker.
        match self.workers[thread_id].pop() {
            Some(task) => {
                // Use sliding window to update worker's latency.
                // The current latency is calculated by the wait time of the task.
                #[cfg(feature = "use_latency")]
                {
                    let latency = self.current_epochs() - task.sched_info().enqueue_epochs();
                    self.workers[thread_id].update_latency(latency);
                }

                Some(task)
            }
            None => {
                // Use sliding window to update worker's latency.
                // We think the current latency is 0 when the worker is idle.
                #[cfg(feature = "use_latency")]
                self.workers[thread_id].update_latency(0);
                None
            }
        }
    }
}
