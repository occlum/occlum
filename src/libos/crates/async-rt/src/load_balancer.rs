use crate::executor::num_vcpus;
use crate::prelude::*;
use crate::scheduler::SchedEntity;
use crate::scheduler::Scheduler;
use crate::task::{SpawnOptions, Task};
use crate::wait::{Waiter, WaiterQueue};
use alloc::collections::VecDeque;

/// A load balancer.
///
/// A load balancer spawns per-VCPU tasks that periodically migrate
/// tasks from vCPUs of higher CPU load to those of lower CPU load.
/// The CPU load is defined as the number of runnable tasks on a vCPU.
pub struct LoadBalancer {
    // The state shared between all migration tasks
    shared: Arc<Shared>,
}

struct Shared {
    scheduler: Arc<Scheduler<Task>>,
    // Whether the migration tasks should stop
    should_stop: AtomicBool,
    // Notify the migration tasks to stop
    stop_wq: WaiterQueue,
}

impl LoadBalancer {
    /// Create a load balancer given the scheduler that is currently in
    /// charge of scheduling tasks of async_rt.
    pub fn new(scheduler: Arc<Scheduler<Task>>) -> Self {
        let shared = Arc::new({
            Shared {
                scheduler,
                should_stop: AtomicBool::new(false),
                stop_wq: WaiterQueue::new(),
            }
        });
        Self { shared }
    }

    /// Start the migration tasks for load balancing.
    pub fn start(&self) {
        debug!("start load balancer");
        let task = MigrationTask::new(self.shared.clone());
        SpawnOptions::new(async move {
            task.run().await;
        })
        .spawn();
    }

    /// Stop the migration tasks.
    pub fn stop(&self) {
        let should_stop = &self.shared.should_stop;
        should_stop.store(true, Ordering::Relaxed);
        self.shared.stop_wq.wake_all();
    }
}

struct MigrationTask {
    shared: Arc<Shared>,
}

impl MigrationTask {
    const WINDOW_SIZE: usize = 4;
    const INTERVAL_MS: u32 = 100;

    pub fn new(shared: Arc<Shared>) -> Self {
        Self { shared }
    }

    pub async fn run(self) {
        let mut waiter = Waiter::new();
        let stop_wq = &self.shared.stop_wq;
        stop_wq.enqueue(&mut waiter);
        while !self.shared.should_stop.load(Ordering::Relaxed) {
            debug!("start migration");
            let num_migrated_tasks = self.do_migration();
            debug!("migrated {:?} tasks", num_migrated_tasks);

            let mut timeout = Duration::from_millis(Self::INTERVAL_MS.into());
            let _ = waiter.wait_timeout(Some(&mut timeout)).await;
            waiter.reset();
        }
        stop_wq.dequeue(&mut waiter);
    }

    fn do_migration(&self) -> i32 {
        // Find vCPUs that are less busy than this vCPU. We only migrate
        // tasks from this vCPU to less busy vCPUs.
        let local_schedulers = &self.shared.scheduler.local_schedulers;

        let mut vcpus_load: VecDeque<(u32, usize)> = (0..num_vcpus())
            .map(|vcpu| {
                let load = local_schedulers[vcpu as usize].len();
                (vcpu, load)
            })
            .collect();

        vcpus_load.make_contiguous().sort_unstable_by(|a, b| {
            let load_a = a.1;
            let load_b = b.1;
            load_a.partial_cmp(&load_b).unwrap()
        });

        let (src_vcpu, src_load) = vcpus_load.pop_back().unwrap();
        let (dst_vcpu, dst_load) = vcpus_load.pop_front().unwrap();

        if src_load <= dst_load + 3 {
            return 0_i32;
        }

        let max_tasks_to_migrate = (src_load - dst_load) / 2;
        let mut drained_vec = Vec::with_capacity(max_tasks_to_migrate);

        let src_scheduler = &local_schedulers[src_vcpu as usize];
        let dst_scheduler = &local_schedulers[dst_vcpu as usize];

        let num_migrated_tasks = src_scheduler.drain(
            |task| {
                // Need to respect affinity when doing migration
                let affinity = task.sched_state().affinity();
                affinity.get(dst_vcpu as usize)
            },
            &mut drained_vec,
        );

        for task in drained_vec {
            dst_scheduler.enqueue(&task);
        }

        num_migrated_tasks as i32
    }
}
