use super::queue::TaskQueue;
use crate::prelude::*;
use crate::sched::SchedPriority;
use crate::task::Task;

pub(crate) struct Injector {
    high_pri_queue: TaskQueue,
    normal_pri_queue: TaskQueue,
    low_pri_queue: TaskQueue,
}

impl Injector {
    pub fn new() -> Self {
        Self {
            high_pri_queue: TaskQueue::new(None),
            normal_pri_queue: TaskQueue::new(None),
            low_pri_queue: TaskQueue::new(None),
        }
    }

    pub fn push(&self, task: Arc<Task>) {
        self.queue(task.sched_info().priority()).push(task).unwrap();
    }

    pub fn pop(&self) -> Option<Arc<Task>> {
        if let Some(task) = self.high_pri_queue.pop() {
            return Some(task);
        }

        if let Some(task) = self.normal_pri_queue.pop() {
            return Some(task);
        }

        if let Some(task) = self.low_pri_queue.pop() {
            return Some(task);
        }

        None
    }

    pub fn pop_with_priority(&self, priority: SchedPriority) -> Option<Arc<Task>> {
        self.queue(priority).pop()
    }

    pub fn len(&self, priority: SchedPriority) -> usize {
        self.queue(priority).len()
    }

    pub fn is_empty(&self, priority: SchedPriority) -> bool {
        self.queue(priority).is_empty()
    }

    fn queue(&self, priority: SchedPriority) -> &TaskQueue {
        match priority {
            SchedPriority::High => &self.high_pri_queue,
            SchedPriority::Normal => &self.normal_pri_queue,
            SchedPriority::Low => &self.low_pri_queue,
        }
    }
}
