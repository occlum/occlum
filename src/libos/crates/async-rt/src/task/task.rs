use alloc::sync::Weak;
use core::fmt::{self, Debug};

use futures::task::ArcWake;

use crate::executor::EXECUTOR;
use crate::prelude::*;
use crate::sched::{SchedInfo, SchedPriority};
use crate::task::{LocalsMap, TaskId, Tirqs};

const DEFAULT_BUDGET: u8 = 64;

pub struct Task {
    tid: TaskId,
    sched_info: SchedInfo,
    future: Mutex<Option<BoxFuture<'static, ()>>>,
    locals: LocalsMap,
    budget: u8,
    consumed_budget: AtomicU8,
    tirqs: Tirqs,
    weak_self: Weak<Self>,
}

impl Task {
    pub fn tid(&self) -> TaskId {
        self.tid
    }

    pub fn sched_info(&self) -> &SchedInfo {
        &self.sched_info
    }

    pub fn tirqs(&self) -> &Tirqs {
        &self.tirqs
    }

    /// Get the task that a given tirqs is associated to.
    ///
    /// # Safety
    ///
    /// This behavior of this function is undefined if the given tirqs is not
    /// a field of a task.
    pub(crate) unsafe fn from_tirqs(tirqs: &Tirqs) -> Arc<Self> {
        use intrusive_collections::container_of;

        let tirqs_ptr = tirqs as *const _;
        // Safety. The pointer is valid and the field-container relationship is hold
        let task_ptr = unsafe { container_of!(tirqs_ptr, Task, tirqs) };
        // Safety. The container's pointer is valid as long as the field's pointer is valid.
        let task = unsafe { &*task_ptr };
        task.to_arc()
    }

    pub(crate) fn future(&self) -> &Mutex<Option<BoxFuture<'static, ()>>> {
        &self.future
    }

    pub(crate) fn locals(&self) -> &LocalsMap {
        &self.locals
    }

    pub(crate) fn has_remained_budget(&self) -> bool {
        self.consumed_budget.load(Ordering::Relaxed) < self.budget
    }

    pub(crate) fn reset_budget(&self) {
        self.consumed_budget.store(0, Ordering::Relaxed);
    }

    pub(crate) fn consume_budget(&self) {
        self.consumed_budget.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn to_arc(&self) -> Arc<Self> {
        self.weak_self.upgrade().unwrap()
    }
}

unsafe impl Sync for Task {}

impl Drop for Task {
    fn drop(&mut self) {
        // Drop the locals explicitly so that we can take care of any potential panics
        // here. One possible reason of panic is the drop method of a task-local variable
        // requires accessinng another already-dropped task-local variable.
        // TODO: handle panic
        unsafe {
            self.locals.clear();
        }
    }
}

impl ArcWake for Task {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        EXECUTOR.wake_task(arc_self.clone());
    }
}

impl Debug for Task {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Task").field("tid", &self.tid).finish()
    }
}

pub struct TaskBuilder {
    future: Option<BoxFuture<'static, ()>>,
    priority: SchedPriority,
    budget: u8,
}

impl TaskBuilder {
    pub fn new(future: impl Future<Output = ()> + 'static + Send) -> Self {
        Self {
            future: Some(future.boxed()),
            priority: SchedPriority::Normal,
            budget: DEFAULT_BUDGET,
        }
    }

    pub fn priority(mut self, priority: SchedPriority) -> Self {
        self.priority = priority;
        self
    }

    pub fn budget(mut self, budget: u8) -> Self {
        self.budget = budget;
        self
    }

    pub fn build(&mut self) -> Arc<Task> {
        assert!(self.future.is_some());

        let tid = TaskId::new();
        let sched_info = SchedInfo::new(self.priority);
        let future = Mutex::new(self.future.take());
        let locals = LocalsMap::new();
        let budget = self.budget;
        let consumed_budget = AtomicU8::new(0);
        // Safety. The tirqs will be inserted into a Task before using it.
        let tirqs = unsafe { Tirqs::new() };
        let weak_self = Weak::new();
        let task = Task {
            tid,
            sched_info,
            future,
            locals,
            budget,
            consumed_budget,
            tirqs,
            weak_self,
        };
        // Create an Arc and update the weak_self
        new_self_ref_arc::new_self_ref_arc!(task)
    }
}
