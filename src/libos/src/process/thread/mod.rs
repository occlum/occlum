use std::fmt;
use std::ptr::NonNull;

use super::task::Task;
use super::{
    FileTableRef, FsViewRef, ProcessRef, ProcessVM, ProcessVMRef, ResourceLimitsRef, ThreadRef,
};
use crate::prelude::*;

pub use self::builder::ThreadBuilder;
pub use self::id::ThreadId;

mod builder;
mod id;

pub struct Thread {
    // Low-level info
    task: Task,
    // Immutable info
    tid: ThreadId,
    // Mutable info
    clear_ctid: SgxRwLock<Option<NonNull<pid_t>>>,
    inner: SgxMutex<ThreadInner>,
    // Process
    process: ProcessRef,
    // Resources
    vm: ProcessVMRef,
    fs: FsViewRef,
    files: FileTableRef,
    rlimits: ResourceLimitsRef,
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum ThreadStatus {
    Init,
    Running,
    Exited,
}

impl Thread {
    pub fn process(&self) -> &ProcessRef {
        &self.process
    }

    pub fn task(&self) -> &Task {
        &self.task
    }

    pub fn tid(&self) -> pid_t {
        self.tid.as_u32()
    }

    pub fn status(&self) -> ThreadStatus {
        self.inner().status()
    }

    pub fn vm(&self) -> &ProcessVMRef {
        &self.vm
    }

    pub fn files(&self) -> &FileTableRef {
        &self.files
    }

    /// Get a file from the file table.
    pub fn file(&self, fd: FileDesc) -> Result<FileRef> {
        self.files().lock().unwrap().get(fd)
    }

    /// Add a file to the file table.
    pub fn add_file(&self, new_file: FileRef, close_on_spawn: bool) -> FileDesc {
        self.files().lock().unwrap().put(new_file, close_on_spawn)
    }

    pub fn fs(&self) -> &FsViewRef {
        &self.fs
    }

    pub fn rlimits(&self) -> &ResourceLimitsRef {
        &self.rlimits
    }

    pub fn clear_ctid(&self) -> Option<NonNull<pid_t>> {
        *self.clear_ctid.read().unwrap()
    }

    pub fn set_clear_ctid(&self, new_clear_ctid: Option<NonNull<pid_t>>) {
        *self.clear_ctid.write().unwrap() = new_clear_ctid;
    }

    pub(super) fn start(&self, host_tid: pid_t) {
        self.inner().start(host_tid);
    }

    pub(super) fn exit(&self, exit_status: i32) -> usize {
        // Remove this thread from its owner process
        let mut process_inner = self.process.inner();
        let threads = process_inner.threads_mut().unwrap();
        let thread_i = threads
            .iter()
            .position(|thread| thread.tid() == self.tid())
            .expect("the thread must belong to the process");
        threads.swap_remove(thread_i);

        self.inner().exit(exit_status);

        threads.len()
    }

    pub(super) fn inner(&self) -> SgxMutexGuard<ThreadInner> {
        self.inner.lock().unwrap()
    }
}

impl PartialEq for Thread {
    fn eq(&self, other: &Self) -> bool {
        self.tid() == other.tid()
    }
}

// Why manual implementation of Debug trait?
//
// An explict implementation of Debug trait is required since Process and Thread
// structs refer to each other. Thus, the automatically-derived implementation
// of Debug trait for the two structs may lead to infinite loop.

impl fmt::Debug for Thread {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Thread")
            .field("tid", &self.tid())
            .field("pid", &self.process().pid())
            .field("inner", &self.inner())
            .field("vm", self.vm())
            .field("fs", self.fs())
            .field("files", self.files())
            .finish()
    }
}

unsafe impl Send for Thread {}
unsafe impl Sync for Thread {}

#[derive(Debug)]
pub enum ThreadInner {
    Init,
    Live { host_tid: pid_t },
    Exited { exit_status: i32 },
}

impl ThreadInner {
    pub fn new() -> Self {
        Self::Init
    }

    pub fn status(&self) -> ThreadStatus {
        match self {
            Self::Init { .. } => ThreadStatus::Init,
            Self::Live { .. } => ThreadStatus::Running,
            Self::Exited { .. } => ThreadStatus::Exited,
        }
    }

    pub fn exit_status(&self) -> Option<i32> {
        match self {
            Self::Exited { exit_status } => Some(*exit_status),
            _ => None,
        }
    }

    pub fn host_tid(&self) -> Option<pid_t> {
        match self {
            Self::Live { host_tid } => Some(*host_tid),
            _ => None,
        }
    }

    pub fn start(&mut self, host_tid: pid_t) {
        debug_assert!(self.status() == ThreadStatus::Init);
        *self = Self::Live { host_tid };
    }

    pub fn exit(&mut self, exit_status: i32) {
        debug_assert!(self.status() == ThreadStatus::Running);
        *self = Self::Exited { exit_status };
    }
}
