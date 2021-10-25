use crate::prelude::*;
use crate::task::Task;

pub const MAX_QUEUED_TASKS: usize = 1_000;

pub trait Scheduler: Send + Sync {
    fn enqueue_task(&self, task: Arc<Task>);
    fn dequeue_task(&self, thread_id: usize) -> Option<Arc<Task>>;
}
