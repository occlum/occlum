use futures::task::waker_ref;

use crate::config::CONFIG;
use crate::parks::Parks;
use crate::prelude::*;
#[allow(unused_imports)]
use crate::sched::{BasicScheduler, PriorityScheduler, Scheduler};
use crate::task::Task;

pub fn parallelism() -> u32 {
    EXECUTOR.parallelism()
}

// Returning number of running vcpus
pub fn run_tasks() -> u32 {
    EXECUTOR.run_tasks()
}

pub fn shutdown() {
    EXECUTOR.shutdown()
}

lazy_static! {
    pub(crate) static ref EXECUTOR: Executor = {
        let parallelism = CONFIG.parallelism();
        Executor::new(parallelism).unwrap()
    };
}

pub(crate) struct Executor {
    parallelism: u32,
    running_vcpu_num: AtomicU32,
    next_thread_id: AtomicU32,
    is_shutdown: AtomicBool,
    parks: Arc<Parks>,
    scheduler: Box<dyn Scheduler>,
}

impl Executor {
    pub fn new(parallelism: u32) -> Result<Self> {
        if parallelism == 0 {
            return_errno!(EINVAL, "invalid argument");
        }

        let next_thread_id = AtomicU32::new(0);
        let running_vcpu_num = AtomicU32::new(0);
        let is_shutdown = AtomicBool::new(false);
        let parks = Arc::new(Parks::new(parallelism));
        let scheduler = Box::new(BasicScheduler::new(parks.clone()));
        // let scheduler = Box::new(PriorityScheduler::new(parks.clone()));

        let new_self = Self {
            parallelism,
            running_vcpu_num,
            next_thread_id,
            is_shutdown,
            parks,
            scheduler,
        };
        Ok(new_self)
    }

    pub fn parallelism(&self) -> u32 {
        self.parallelism
    }

    pub fn run_tasks(&self) -> u32 {
        let thread_id = self.next_thread_id.fetch_add(1, Ordering::Relaxed) as usize;
        assert!(thread_id < self.parallelism as usize);
        self.running_vcpu_num.fetch_add(1, Ordering::Relaxed);

        crate::task::current::set_vcpu_id(thread_id as u32);
        debug!("run tasks on vcpu {}", thread_id);

        self.parks.register(thread_id);

        // The max number of dequeue retries before go to sleep
        const MAX_DEQUEUE_RETRIES: usize = 5000;
        let mut dequeue_retries = 0;
        loop {
            let task_option = self.scheduler.dequeue_task(thread_id);

            // Stop the executor iff all the ready tasks are executed
            if self.is_shutdown() && task_option.is_none() {
                let num = self.running_vcpu_num.fetch_sub(1, Ordering::Relaxed) as u32;
                assert!(num >= 1);
                self.parks.unregister(thread_id);
                return num - 1;
            }

            match task_option {
                Some(task) => {
                    dequeue_retries = 0;

                    task.reset_enqueued();
                    self.execute_task(task)
                }
                None => {
                    dequeue_retries += 1;
                    if dequeue_retries >= MAX_DEQUEUE_RETRIES {
                        self.parks.park();
                        dequeue_retries = 0;
                    }
                }
            }
        }
    }

    pub fn execute_task(&self, task: Arc<Task>) {
        // Keep the lock to avoid race contidion in yield process.
        let mut future_slot = task.future().lock();
        let mut future = match future_slot.take() {
            None => {
                return;
            }
            Some(future) => future,
        };

        crate::task::current::set(task.clone());

        task.consume_budget();

        let waker = waker_ref(&task);
        let context = &mut Context::from_waker(&*waker);
        if let Poll::Pending = future.as_mut().poll(context) {
            *future_slot = Some(future);
        }

        crate::task::current::reset();
    }

    /// Accept a new task and schedule it.
    pub fn accept_task(&self, task: Arc<Task>) {
        if self.is_shutdown() {
            panic!("a shut-down executor cannot spawn new tasks");
        }

        task.try_set_enqueued().unwrap();
        self.scheduler.enqueue_task(task);
    }

    /// Wake up an old task and schedule it.
    pub fn wake_task(&self, task: &Arc<Task>) {
        if self.is_shutdown() {
            // TODO: What to do if there are still task in the run queues
            // of the scheduler when the executor is shutdown.
            // e.g., yield-loop tasks might be waken up when the executer
            // is shutdown.
            debug!("task {:?} is running when executor shut-down", task.tid());
            return;
        }

        // Avoid a task from consuming the limited space of the queues of
        // the underlying scheduler due to the task being enqueued multiple
        // times
        if let Err(_) = task.try_set_enqueued() {
            return;
        }

        self.scheduler.enqueue_task(task.clone());
    }

    pub fn shutdown(&self) {
        self.is_shutdown.store(true, Ordering::Relaxed);

        self.parks.unpark_all();
        crate::time::wake_timer_wheel(&Duration::default()); // wake the time wheel right now
    }

    pub fn is_shutdown(&self) -> bool {
        self.is_shutdown.load(Ordering::Relaxed)
    }
}
