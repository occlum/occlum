use core::fmt::{self, Debug};

use futures::task::ArcWake;

use crate::executor::EXECUTOR;
use crate::prelude::*;
use crate::sched::SchedInfo;
use crate::task::{LocalsMap, TaskId};

pub struct Task {
    tid: TaskId,
    sched_info: SchedInfo,
    future: Mutex<Option<BoxFuture<'static, ()>>>,
    locals: LocalsMap,
}

impl Task {
    pub fn new(future: impl Future<Output = ()> + 'static + Send) -> Self {
        let tid = TaskId::new();
        let sched_info = SchedInfo::new();
        let future = Mutex::new(Some(future.boxed()));
        let locals = LocalsMap::new();
        Self {
            tid,
            sched_info,
            future,
            locals,
        }
    }

    pub fn tid(&self) -> TaskId {
        self.tid
    }

    pub fn sched_info(&self) -> &SchedInfo {
        &self.sched_info
    }

    pub(crate) fn future(&self) -> &Mutex<Option<BoxFuture<'static, ()>>> {
        &self.future
    }

    pub(crate) fn locals(&self) -> &LocalsMap {
        &self.locals
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
        EXECUTOR.accept_task(arc_self.clone());
    }
}

impl Debug for Task {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Task").field("tid", &self.tid).finish()
    }
}
