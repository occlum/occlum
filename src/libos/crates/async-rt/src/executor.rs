use futures::task::waker_ref;

use crate::load_balancer::LoadBalancer;
use crate::prelude::*;
#[allow(unused_imports)]
use crate::task::Task;
use crate::vcpu;

use crate::scheduler::{SchedEntity, Scheduler, StatusNotifier};

/// Returning number of running vcpus
pub fn num_vcpus() -> u32 {
    EXECUTOR.num_vcpus()
}

/// Start running tasks in this vcpu of executor
pub fn run_tasks() -> u32 {
    EXECUTOR.run_tasks()
}

/// Shutdown the executor
pub fn shutdown() {
    EXECUTOR.shutdown()
}

/// Start the load balancer
pub fn start_load_balancer() {
    // EXECUTOR.load_balancer.start();
}

lazy_static! {
    pub(crate) static ref EXECUTOR: Executor = {
        let num_vcpus = vcpu::get_total();
        Executor::new(num_vcpus).unwrap()
    };
}

pub(crate) struct Executor {
    num_vcpus: u32,
    running_vcpus: AtomicU32,
    scheduler: Arc<Scheduler<Task>>,
    is_shutdown: AtomicBool,
    load_balancer: LoadBalancer,
    // Todo: calculate the number of pending tasks in vcpus, integrate it into vcpu selecting mechanism
    // vcpus_pending_len: Box<[AtomicU32]>,
}

impl Executor {
    /// Initialize executor
    pub fn new(num_vcpus: u32) -> Result<Self> {
        if num_vcpus == 0 {
            return_errno!(EINVAL, "invalid argument");
        }

        let running_vcpus = AtomicU32::new(0);
        let scheduler = Arc::new(Scheduler::new(num_vcpus));
        let is_shutdown = AtomicBool::new(false);
        let load_balancer = LoadBalancer::new(scheduler.clone());

        // Init the vector of pending tasks number per vcpus
        // let vcpus_pending_len = {
        //     let mut vcpus_pending_len = Vec::with_capacity(num_vcpus as usize);
        //     for _ in 0..num_vcpus {
        //         vcpus_pending_len.push(AtomicU32::new(0));
        //     }
        //     vcpus_pending_len.into_boxed_slice()
        // };

        let new_self = Self {
            num_vcpus,
            running_vcpus,
            scheduler,
            is_shutdown,
            load_balancer,
            // vcpus_pending_len,
        };
        Ok(new_self)
    }

    /// Return the number of vcpus in the executor
    pub fn num_vcpus(&self) -> u32 {
        self.num_vcpus
    }

    /// Start running tasks in this vcpu of executor
    pub fn run_tasks(&self) -> u32 {
        let this_vcpu = self.running_vcpus.fetch_add(1, Ordering::Relaxed);
        debug_assert!(this_vcpu < self.num_vcpus);

        vcpu::set_current(this_vcpu);

        let this_local_scheduler = &self.scheduler.local_schedulers[this_vcpu as usize];

        loop {
            let task = match self.dequeue_wait() {
                Some(entity) => entity,
                None => {
                    break;
                }
            };

            let mut future_slot = match task.future().try_lock() {
                Some(future) => future,
                None => {
                    // The task happens to be executed by other vCPUs at the moment.
                    // Try to execute it later.
                    debug!("the task happens to be executed by other vcpus, re-enqueue it");
                    self.scheduler.enqueue(&task);
                    continue;
                }
            };

            let future = match future_slot.as_mut() {
                Some(future) => future,
                None => {
                    debug!("the task happens to be completed, task: {:?}", task.tid());
                    // The task happens to be completed
                    continue;
                }
            };

            crate::task::current::set(task.clone());

            let start = task.sched_state().update_exec_start();

            // Execute task
            let waker = waker_ref(&task);
            let context = &mut Context::from_waker(&*waker);
            let ret = future.as_mut().poll(context);

            match ret {
                Poll::Ready(()) => {
                    // As the task is completed, we can destory the future
                    drop(future_slot.take());
                }
                Poll::Pending => {
                    let stop = task.sched_state().update_exec_stop();
                    let elapsed = stop.duration_since(start).as_millis() as usize;

                    let remain_ms = task.sched_state().elapse(elapsed as u32);
                    if remain_ms == 0 {
                        task.sched_state().report_preemption();
                    }
                }
            }
            drop(future_slot);

            if this_local_scheduler.blocking_num() == 0 {
                self.scheduler
                    .vcpu_selector
                    .notify_heavy_status(this_vcpu, false);
            }

            // Reset current task
            crate::task::current::reset();
        }

        vcpu::clear_current();
        let num = self.running_vcpus.load(Ordering::Relaxed);
        num - 1
    }

    /// Dequeue task. If no task in current vcpu, wait for enqueueing.
    /// Return None if the executor has been shutdown.
    #[inline]
    fn dequeue_wait(&self) -> Option<Arc<Task>> {
        let this_vcpu = vcpu::get_current().unwrap();
        loop {
            if self.is_shutdown() {
                return None;
            }

            let entity = self.scheduler.dequeue();
            if entity.is_some() {
                return entity;
            }

            // Firstly enter idle state waiting for tasks.
            // If the idle time has been depleted, make this vcpu enter sleep state.
            self.scheduler.local_schedulers[this_vcpu as usize].wait_enqueue();
        }
    }

    /// Accept a new task and schedule it, mainly called by spawn
    pub fn accept_task(&self, task: Arc<Task>) {
        assert!(
            !self.is_shutdown(),
            "a shut-down executor cannot spawn new tasks"
        );
        self.schedule_task(&task);
    }

    /// Wake up an old task and schedule it, mainly called by wake_by_ref
    pub fn wake_task(&self, task: &Arc<Task>) {
        if self.is_shutdown() {
            // TODO: What to do if there are still task in the run queues
            // of the scheduler when the executor is shutdown.
            // e.g., yield-loop tasks might be waken up when the executer
            // is shutdown.
            debug!("task {:?} is running when executor shut-down", task.tid());
            return;
        }

        // the task has been enqueued, not neccessary to enqueue more than once
        if task.sched_state().is_enqueued() {
            return;
        }

        // When waking up a pending tasks in vcpu,
        // the pending task number of corresponding vcpus should subtract one
        // if let Some(last_vcpu) = task.sched_state().vcpu() {
        //     self.vcpus_pending_len[last_vcpu as usize].fetch_sub(1, Ordering::Relaxed);
        // }

        self.schedule_task(task);
    }

    /// Re-schedule task
    pub fn schedule_task(&self, task: &Arc<Task>) {
        self.scheduler.enqueue(task);
    }

    /// Check the shutdown status of executor
    pub fn is_shutdown(&self) -> bool {
        self.is_shutdown.load(Ordering::Relaxed)
    }

    /// Shutdown executor and wake threads of all vcpus and timer
    pub fn shutdown(&self) {
        self.stop_load_balancer();
        self.is_shutdown.store(true, Ordering::Relaxed);

        vcpu::unpark_all();
        // wake the time wheel right now
        crate::time::wake_timer_wheel(&Duration::default());
    }

    /// Stop the load balancer.
    /// Todo: reimplement and enable load balancer for improving scheduler performance
    pub fn stop_load_balancer(&self) {
        // self.load_balancer.stop();
    }
}
