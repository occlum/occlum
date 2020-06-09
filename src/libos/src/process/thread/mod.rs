use std::fmt;
use std::ptr::NonNull;

use super::task::Task;
use super::{
    FileTableRef, ForcedExitStatus, FsViewRef, ProcessRef, ProcessVM, ProcessVMRef,
    ResourceLimitsRef, SchedAgentRef, TermStatus, ThreadRef,
};
use crate::prelude::*;
use crate::signal::{SigQueues, SigSet, SigStack};
use crate::time::ThreadProfiler;

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
    sched: SchedAgentRef,
    rlimits: ResourceLimitsRef,
    // Signal
    sig_queues: SgxMutex<SigQueues>,
    sig_mask: SgxRwLock<SigSet>,
    sig_tmp_mask: SgxRwLock<SigSet>,
    sig_stack: SgxMutex<Option<SigStack>>,
    // System call timing
    profiler: SgxMutex<Option<ThreadProfiler>>,
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

    pub fn sched(&self) -> &SchedAgentRef {
        &self.sched
    }

    /// Get the signal queues for thread-directed signals.
    pub fn sig_queues(&self) -> &SgxMutex<SigQueues> {
        &self.sig_queues
    }

    /// Get the per-thread signal mask.
    pub fn sig_mask(&self) -> &SgxRwLock<SigSet> {
        &self.sig_mask
    }

    /// Get the per-thread, temporary signal mask.
    ///
    /// The tmp mask is always cleared at the end of the execution
    /// of a syscall.
    pub fn sig_tmp_mask(&self) -> &SgxRwLock<SigSet> {
        &self.sig_tmp_mask
    }

    /// Get the alternate signal stack.
    pub fn sig_stack(&self) -> &SgxMutex<Option<SigStack>> {
        &self.sig_stack
    }

    /// Get the alternate thread performance profiler
    pub fn profiler(&self) -> &SgxMutex<Option<ThreadProfiler>> {
        &self.profiler
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
        self.sched().lock().unwrap().attach(host_tid);
        self.inner().start();

        #[cfg(feature = "syscall_timing")]
        self.profiler()
            .lock()
            .unwrap()
            .as_mut()
            .unwrap()
            .start()
            .unwrap();
    }

    pub(super) fn exit(&self, term_status: TermStatus) -> usize {
        #[cfg(feature = "syscall_timing")]
        self.profiler()
            .lock()
            .unwrap()
            .as_mut()
            .unwrap()
            .stop()
            .unwrap();

        self.sched().lock().unwrap().detach();

        // Remove this thread from its owner process
        let mut process_inner = self.process.inner();
        let threads = process_inner.threads_mut().unwrap();
        let thread_i = threads
            .iter()
            .position(|thread| thread.tid() == self.tid())
            .expect("the thread must belong to the process");
        threads.swap_remove(thread_i);

        self.inner().exit(term_status);

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
            .field("profiler", self.profiler())
            .finish()
    }
}

unsafe impl Send for Thread {}
unsafe impl Sync for Thread {}

#[derive(Debug)]
pub enum ThreadInner {
    Init,
    Running,
    Exited { term_status: TermStatus },
}

impl ThreadInner {
    pub fn new() -> Self {
        Self::Init
    }

    pub fn status(&self) -> ThreadStatus {
        match self {
            Self::Init { .. } => ThreadStatus::Init,
            Self::Running { .. } => ThreadStatus::Running,
            Self::Exited { .. } => ThreadStatus::Exited,
        }
    }

    pub fn term_status(&self) -> Option<TermStatus> {
        match self {
            Self::Exited { term_status } => Some(*term_status),
            _ => None,
        }
    }

    pub fn start(&mut self) {
        debug_assert!(self.status() == ThreadStatus::Init);
        *self = Self::Running;
    }

    pub fn exit(&mut self, term_status: TermStatus) {
        debug_assert!(self.status() == ThreadStatus::Running);
        *self = Self::Exited { term_status };
    }
}
