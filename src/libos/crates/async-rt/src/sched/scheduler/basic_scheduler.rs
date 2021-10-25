use crate::parks::Parks;
use crate::prelude::*;
use crate::sched::Affinity;
use crate::task::Task;

use super::{Scheduler, MAX_QUEUED_TASKS};

use flume::{Receiver, Sender};

pub struct BasicScheduler {
    parallelism: usize,
    run_queues: Vec<Receiver<Arc<Task>>>,
    task_senders: Vec<Sender<Arc<Task>>>,
    parks: Arc<Parks>,
}

impl BasicScheduler {
    pub fn new(parks: Arc<Parks>) -> Self {
        let parallelism = parks.len();
        let mut run_queues = Vec::with_capacity(parallelism);
        let mut task_senders = Vec::with_capacity(parallelism);
        for _ in 0..parallelism {
            let (task_sender, run_queue) = flume::bounded(MAX_QUEUED_TASKS);
            run_queues.push(run_queue);
            task_senders.push(task_sender);
        }

        Self {
            parallelism,
            run_queues,
            task_senders,
            parks,
        }
    }
}

impl Scheduler for BasicScheduler {
    fn enqueue_task(&self, task: Arc<Task>) {
        let affinity = task.sched_info().affinity().read();
        assert!(!affinity.is_empty());
        let mut thread_id = task.sched_info().last_thread_id() as usize;
        while !affinity.get(thread_id) {
            thread_id = (thread_id + 1) % Affinity::max_threads();
        }
        drop(affinity);

        task.sched_info().set_last_thread_id(thread_id as u32);
        self.task_senders[thread_id]
            .send(task)
            .expect("too many tasks enqueued");

        self.parks.unpark(thread_id);
    }

    fn dequeue_task(&self, thread_id: usize) -> Option<Arc<Task>> {
        self.run_queues[thread_id].try_recv().ok()
    }
}
