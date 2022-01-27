use crate::parks::Parks;
use crate::prelude::*;
use crate::sched::Affinity;
use crate::task::Task;

use super::{Scheduler, MAX_QUEUED_TASKS};

use flume::{Receiver, Sender, TrySendError};

lazy_static! {
    static ref PENDING_TASKS: Mutex<VecDeque<Arc<Task>>> = Mutex::new(VecDeque::new());
    static ref HAS_PENDING: AtomicBool = AtomicBool::new(false);
}

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
        let res = self.task_senders[thread_id].try_send(task);

        match res {
            Ok(()) => {
                self.parks.unpark(thread_id);
            }
            Err(TrySendError::Full(task)) => {
                let mut pending_tasks = PENDING_TASKS.lock();
                pending_tasks.push_back(task);
                HAS_PENDING.store(true, Ordering::Relaxed);
            }
            _ => panic!("task queue disconnected"),
        }
    }

    fn dequeue_task(&self, thread_id: usize) -> Option<Arc<Task>> {
        let res = self.run_queues[thread_id].try_recv().ok();

        // If there is any pending task, try to enqueue it
        if HAS_PENDING.load(Ordering::Relaxed) == true {
            let mut pending_tasks = PENDING_TASKS.lock();
            let task = pending_tasks.pop_front();
            drop(pending_tasks);

            if let Some(task) = task {
                self.enqueue_task(task);
            } else {
                HAS_PENDING.store(false, Ordering::Relaxed);
            }
        }

        res
    }
}

impl Drop for BasicScheduler {
    fn drop(&mut self) {
        let pending_tasks = PENDING_TASKS.lock();
        if pending_tasks.len() > 0 {
            panic!("There are some pending tasks.")
        }
    }
}
