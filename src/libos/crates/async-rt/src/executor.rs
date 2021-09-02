use flume::{Receiver, Sender};
use futures::task::waker_ref;

use crate::config::CONFIG;
use crate::parks::Parks;
use crate::prelude::*;
use crate::sched::Affinity;
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
    run_queues: Vec<Receiver<Arc<Task>>>,
    task_senders: Vec<Sender<Arc<Task>>>,
    next_run_queue_id: AtomicU32,
    is_shutdown: AtomicBool,
    parks: Parks,
}

impl Executor {
    pub fn new(parallelism: u32) -> Result<Self> {
        if parallelism == 0 {
            return_errno!(EINVAL, "invalid argument");
        }

        const MAX_QUEUED_TASKS: usize = 1_000;
        let mut run_queues = Vec::with_capacity(parallelism as usize);
        let mut task_senders = Vec::with_capacity(parallelism as usize);
        for _ in 0..parallelism {
            let (task_sender, run_queue) = flume::bounded(MAX_QUEUED_TASKS);
            run_queues.push(run_queue);
            task_senders.push(task_sender);
        }

        let is_shutdown = AtomicBool::new(false);
        let next_run_queue_id = AtomicU32::new(0);
        let parks = Parks::new(parallelism);

        let new_self = Self {
            parallelism,
            run_queues,
            task_senders,
            next_run_queue_id,
            is_shutdown,
            parks,
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
                let task_res = run_queue.try_recv();

                if self.is_shutdown.load(Ordering::Relaxed) {
                    return;
                }

                match task_res {
                    Err(_) => {
                        self.parks.park_timeout(
                            run_queue_id as usize,
                            core::time::Duration::from_millis(10),
                        );
                        continue;
                    }
                    Ok(task) => task,
                }
            };

            let mut future_slot = task.future().lock();
            let mut future = match future_slot.take() {
                None => continue,
                Some(future) => future,
            };
            drop(future_slot);

            crate::task::current::set(task.clone());

            let waker = waker_ref(&task);
            let context = &mut Context::from_waker(&*waker);
            if let Poll::Pending = future.as_mut().poll(context) {
                let mut future_slot = task.future().lock();
                *future_slot = Some(future);
            }

            crate::task::current::reset();
        }
    }

    pub fn accept_task(&self, task: Arc<Task>) {
        if self.is_shutdown() {
            // Should not panic for now, return directly.
            // LibOS task may be waked up after shutdown, e.g. io_uring task.
            // To solve this problem actually,
            // we can add task_attribute and task_status to task struct,
            // then we can prevent repeated wake-ups
            // we also need store these tasks in executor,
            // shutdown these tasks before shutdown executor.

            // panic!("a shut-down executor cannot spawn new tasks");
            return;
        }

        let thread_id = self.pick_thread_for(&task);
        self.task_senders[thread_id]
            .send(task)
            .expect("too many tasks enqueued");

        self.parks.unpark(thread_id);
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

        self.parks.unpark_all();
    }

    pub fn is_shutdown(&self) -> bool {
        self.is_shutdown.load(Ordering::Relaxed)
    }
}
