use futures::task::waker_ref;

use crate::config::CONFIG;
use crate::parks::Parks;
use crate::prelude::*;
use crate::sched::{BasicScheduler, PriorityScheduler, Scheduler};
use crate::task::Task;

pub fn parallelism() -> u32 {
    EXECUTOR.parallelism()
}

pub fn run_tasks() {
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
        let is_shutdown = AtomicBool::new(false);
        let parks = Arc::new(Parks::new(parallelism));
        let scheduler = Box::new(BasicScheduler::new(parks.clone()));
        // let scheduler = Box::new(PriorityScheduler::new(parks.clone()));

        let new_self = Self {
            parallelism,
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

    pub fn run_tasks(&self) {
        let thread_id = self.next_thread_id.fetch_add(1, Ordering::Relaxed) as usize;
        assert!(thread_id < self.parallelism as usize);

        loop {
            let task_option = self.scheduler.dequeue_task(thread_id);

            if self.is_shutdown() {
                return;
            }

            match task_option {
                Some(task) => self.execute_task(task),
                None => self
                    .parks
                    .park_timeout(thread_id, Duration::from_millis(10)),
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

        self.scheduler.enqueue_task(task);
    }

    /// Wake up a old task and schedule it.
    pub fn wake_task(&self, task: Arc<Task>) {
        if self.is_shutdown() {
            // TODO: What to do if there are still task in the run queues
            // of the scheduler when the executor is shutdown.
            // e.g., yield-loop tasks might be waken up when the executer
            // is shutdown.
            return;
        }

        self.scheduler.enqueue_task(task);
    }

    pub fn shutdown(&self) {
        self.is_shutdown.store(true, Ordering::Relaxed);

        self.parks.unpark_all();
    }

    pub fn is_shutdown(&self) -> bool {
        self.is_shutdown.load(Ordering::Relaxed)
    }
}
