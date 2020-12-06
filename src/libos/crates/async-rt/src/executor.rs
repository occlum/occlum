use crossbeam_queue::ArrayQueue;
use futures::task::waker_ref;

use crate::prelude::*;
use crate::sched::Affinity;
use crate::task::Task;

pub const DEFAULT_PARALLELISM: u32 = 1;

static PARALLELISM: AtomicU32 = AtomicU32::new(DEFAULT_PARALLELISM);

pub fn set_parallelism(parallelism: u32) -> Result<()> {
    if parallelism == 0 {
        return Err("invalid argument");
    }

    PARALLELISM.store(parallelism, Ordering::Relaxed);
    Ok(())
}

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
        let parallelism = PARALLELISM.load(Ordering::Relaxed);
        Executor::new(parallelism).unwrap()
    };
}

pub(crate) struct Executor {
    parallelism: u32,
    run_queues: Vec<ArrayQueue<Arc<Task>>>,
    next_run_queue_id: AtomicU32,
    is_shutdown: AtomicBool,
}

impl Executor {
    pub fn new(parallelism: u32) -> Result<Self> {
        if parallelism == 0 {
            return Err("invalid argument");
        }

        const MAX_QUEUED_TASKS: usize = 1_000;
        let mut run_queues = Vec::with_capacity(parallelism as usize);
        for _ in 0..parallelism {
            let run_queue = ArrayQueue::new(MAX_QUEUED_TASKS);
            run_queues.push(run_queue);
        }

        let is_shutdown = AtomicBool::new(false);
        let next_run_queue_id = AtomicU32::new(0);

        let new_self = Self {
            parallelism,
            run_queues,
            next_run_queue_id,
            is_shutdown,
        };
        Ok(new_self)
    }

    pub fn parallelism(&self) -> u32 {
        self.parallelism
    }

    pub fn run_tasks(&self) {
        let run_queue_id = self.next_run_queue_id.fetch_add(1, Ordering::Relaxed);
        assert!(run_queue_id < self.parallelism);
        let run_queue = &self.run_queues[run_queue_id as usize];
        loop {
            let task = {
                let task_res = run_queue.pop();

                if self.is_shutdown.load(Ordering::Relaxed) {
                    return;
                }

                match task_res {
                    None => {
                        core::sync::atomic::spin_loop_hint();
                        continue;
                    }
                    Some(task) => task,
                }
            };

            let future = task.future();
            let mut future_slot = future.lock();
            let mut future = match future_slot.take() {
                None => continue,
                Some(future) => future,
            };

            crate::task::current::set(task.clone());

            let waker = waker_ref(&task);
            let context = &mut Context::from_waker(&*waker);
            if let Poll::Pending = future.as_mut().poll(context) {
                *future_slot = Some(future);
            }

            crate::task::current::reset();
        }
    }

    pub fn accept_task(&self, task: Arc<Task>) {
        if self.is_shutdown() {
            panic!("a shut-down executor cannot spawn new tasks");
        }

        let thread_id = self.pick_thread_for(&task);
        self.run_queues[thread_id]
            .push(task)
            .expect("too many tasks enqueued");
    }

    fn pick_thread_for(&self, task: &Arc<Task>) -> usize {
        let affinity = task.sched_info().affinity().read();
        assert!(!affinity.is_empty());
        let mut thread_id = task.sched_info().last_thread_id() as usize;
        while !affinity.get(thread_id) {
            thread_id = (thread_id + 1) % Affinity::max_threads();
        }
        drop(affinity);

        task.sched_info().set_last_thread_id(thread_id as u32);
        thread_id
    }

    pub fn shutdown(&self) {
        self.is_shutdown.store(true, Ordering::Relaxed);
    }

    pub fn is_shutdown(&self) -> bool {
        self.is_shutdown.load(Ordering::Relaxed)
    }
}
