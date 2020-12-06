use crate::prelude::*;

#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub struct TaskId(pub u64);

impl TaskId {
    pub fn new() -> Self {
        static NEXT_TASK_ID: AtomicU64 = AtomicU64::new(0);

        let inner = NEXT_TASK_ID.fetch_add(1, Ordering::Relaxed);
        assert!(inner <= u64::max_value() / 2);
        Self(inner)
    }
}
