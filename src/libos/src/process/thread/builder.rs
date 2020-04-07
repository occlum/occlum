use std::ptr::NonNull;

use super::{
    FileTableRef, FsViewRef, ProcessRef, ProcessVM, ProcessVMRef, ResourceLimitsRef, Task, Thread,
    ThreadId, ThreadInner, ThreadRef,
};
use crate::prelude::*;

#[derive(Debug)]
pub struct ThreadBuilder {
    // Mandatory field
    tid: Option<ThreadId>,
    task: Option<Task>,
    process: Option<ProcessRef>,
    vm: Option<ProcessVMRef>,
    // Optional fields
    fs: Option<FsViewRef>,
    files: Option<FileTableRef>,
    rlimits: Option<ResourceLimitsRef>,
    clear_ctid: Option<NonNull<pid_t>>,
}

impl ThreadBuilder {
    pub fn new() -> Self {
        Self {
            tid: None,
            task: None,
            process: None,
            vm: None,
            fs: None,
            files: None,
            rlimits: None,
            clear_ctid: None,
        }
    }

    pub fn tid(mut self, tid: ThreadId) -> Self {
        self.tid = Some(tid);
        self
    }

    pub fn task(mut self, task: Task) -> Self {
        self.task = Some(task);
        self
    }

    pub fn process(mut self, process: ProcessRef) -> Self {
        self.process = Some(process);
        self
    }

    pub fn vm(mut self, vm: ProcessVMRef) -> Self {
        self.vm = Some(vm);
        self
    }

    pub fn fs(mut self, fs: FsViewRef) -> Self {
        self.fs = Some(fs);
        self
    }

    pub fn files(mut self, files: FileTableRef) -> Self {
        self.files = Some(files);
        self
    }

    pub fn rlimits(mut self, rlimits: ResourceLimitsRef) -> Self {
        self.rlimits = Some(rlimits);
        self
    }

    pub fn clear_ctid(mut self, clear_tid_addr: NonNull<pid_t>) -> Self {
        self.clear_ctid = Some(clear_tid_addr);
        self
    }

    pub fn build(self) -> Result<ThreadRef> {
        let tid = self.tid.unwrap_or_else(|| ThreadId::new());
        let task = self
            .task
            .ok_or_else(|| errno!(EINVAL, "task is mandatory"))?;
        let process = self
            .process
            .ok_or_else(|| errno!(EINVAL, "process is mandatory"))?;
        let vm = self
            .vm
            .ok_or_else(|| errno!(EINVAL, "memory is mandatory"))?;
        let fs = self.fs.unwrap_or_default();
        let files = self.files.unwrap_or_default();
        let rlimits = self.rlimits.unwrap_or_default();
        let clear_ctid = SgxRwLock::new(self.clear_ctid);
        let inner = SgxMutex::new(ThreadInner::new());

        let new_thread = Arc::new(Thread {
            task,
            tid,
            clear_ctid,
            inner,
            process,
            vm,
            fs,
            files,
            rlimits,
        });

        let mut inner = new_thread.process().inner();
        inner.threads_mut().unwrap().push(new_thread.clone());
        drop(inner);

        Ok(new_thread)
    }
}
