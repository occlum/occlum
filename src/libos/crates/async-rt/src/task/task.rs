use alloc::sync::Weak;
use core::fmt::{self, Debug};

use futures::task::ArcWake;

use crate::executor::EXECUTOR;
use crate::scheduler::{Priority, SchedEntity, SchedState};
use crate::task::{LocalsMap, TaskId, Tirqs};
use crate::{prelude::*, vcpu};

const DEFAULT_BUDGET: u8 = 64;

pub struct Task {
    tid: TaskId,
    future: Mutex<Option<BoxFuture<'static, ()>>>,
    locals: LocalsMap,
    tirqs: Tirqs,
    weak_self: Weak<Self>,
    sched_state: SchedState,
}

impl SchedEntity for Task {
    fn sched_state(&self) -> &SchedState {
        &self.sched_state
    }
}

impl Task {
    pub fn tid(&self) -> TaskId {
        self.tid
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
        #[allow(unused_unsafe)]
        let task_ptr = unsafe { container_of!(tirqs_ptr, Task, tirqs) };
        // Safety. The container's pointer is valid as long as the field's pointer is valid.
        let task = &*task_ptr;
        task.to_arc()
    }

    pub(crate) fn future(&self) -> &Mutex<Option<BoxFuture<'static, ()>>> {
        &self.future
    }

    pub(crate) fn locals(&self) -> &LocalsMap {
        &self.locals
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
        EXECUTOR.wake_task(arc_self);
    }
}

impl Debug for Task {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Task").field("tid", &self.tid).finish()
    }
}

pub struct TaskBuilder {
    future: Option<BoxFuture<'static, ()>>,
}

// schedstate need to be initialized
impl TaskBuilder {
    pub fn new(future: impl Future<Output = ()> + 'static + Send) -> Self {
        Self {
            future: Some(future.boxed()),
        }
    }
    pub fn build(&mut self) -> Arc<Task> {
        assert!(self.future.is_some());

        let tid = TaskId::new();
        let future = Mutex::new(self.future.take());
        let locals = LocalsMap::new();
        // Safety. The tirqs will be inserted into a Task before using it.
        let tirqs = unsafe { Tirqs::new() };
        let weak_self = Weak::new();

        // need to initialize sched entity
        let vcpus = vcpu::get_total();
        let base_prio = Priority::NORMAL;
        let sched_state = SchedState::new(vcpus, base_prio);

        let task = Task {
            tid,
            future,
            locals,
            tirqs,
            weak_self,
            sched_state,
        };
        // Create an Arc and update the weak_self
        new_self_ref_arc::new_self_ref_arc!(task)
    }
}
