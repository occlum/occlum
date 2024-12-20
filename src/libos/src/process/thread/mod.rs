use spin::Once;
use std::fmt;
use std::ptr::NonNull;

use super::task::Task;
use super::{
    FileTableRef, ForcedExitStatus, FsViewRef, NiceValueRef, ProcessRef, ProcessVM, ProcessVMRef,
    ResourceLimitsRef, RobustListHead, SchedAgentRef, TermStatus, ThreadRef,
};
use crate::events::HostEventFd;
use crate::fs::{EventCreationFlags, EventFile};
use crate::net::AsEpollFile;
use crate::net::THREAD_NOTIFIERS;
use crate::prelude::*;
use crate::signal::{SigQueues, SigSet, SigStack};
use crate::time::ThreadProfiler;
use crate::untrusted::{UntrustedSliceAlloc, UntrustedSliceAllocGuard};

pub use self::builder::ThreadBuilder;
pub use self::id::ThreadId;
pub use self::name::ThreadName;

mod builder;
mod id;
mod name;

pub const IO_BUF_SIZE: usize = 128 * 1024;

pub struct Thread {
    // Low-level info
    task: Task,
    // Immutable info
    tid: ThreadId,
    // Mutable info
    clear_ctid: RwLock<Option<NonNull<pid_t>>>,
    robust_list: RwLock<Option<NonNull<RobustListHead>>>,
    inner: SgxMutex<ThreadInner>,
    name: RwLock<ThreadName>,
    // Process
    process: ProcessRef,
    // Resources
    vm: ProcessVMRef,
    fs: FsViewRef,
    files: FileTableRef,
    sched: SchedAgentRef,
    // According to POSIX, the nice value is a per-process setting.
    // In our implementation, the threads belong to same process
    // share the same nice value.
    nice: NiceValueRef,
    rlimits: ResourceLimitsRef,
    // Signal
    sig_queues: RwLock<SigQueues>,
    sig_mask: RwLock<SigSet>,
    sig_tmp_mask: RwLock<SigSet>,
    sig_stack: SgxMutex<Option<SigStack>>,
    // System call timing
    profiler: SgxMutex<Option<ThreadProfiler>>,
    // Misc
    host_eventfd: Arc<HostEventFd>,
    raw_ptr: RwLock<usize>,
    // Thread ocall buffer
    io_buffer: Once<UntrustedSliceAlloc>,
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum ThreadStatus {
    Init,
    Running,
    Exited,
    ToStop,
    Stopped,
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
    pub fn sig_queues(&self) -> &RwLock<SigQueues> {
        &self.sig_queues
    }

    /// Get the per-thread signal mask.
    pub fn sig_mask(&self) -> &RwLock<SigSet> {
        &self.sig_mask
    }

    /// Get the per-thread, temporary signal mask.
    ///
    /// The tmp mask is always cleared at the end of the execution
    /// of a syscall.
    pub fn sig_tmp_mask(&self) -> &RwLock<SigSet> {
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

    /// Get the host thread's raw pointer of this libos thread
    pub fn raw_ptr(&self) -> usize {
        self.raw_ptr.read().unwrap().clone()
    }

    /// Get a file from the file table.
    pub fn file(&self, fd: FileDesc) -> Result<FileRef> {
        self.files().lock().get(fd)
    }

    /// Add a file to the file table.
    pub fn add_file(&self, new_file: FileRef, close_on_spawn: bool) -> FileDesc {
        self.files().lock().put(new_file, close_on_spawn)
    }

    /// Close a file from the file table. It will release the POSIX advisory locks owned
    /// by current process.
    pub fn close_file(&self, fd: FileDesc) -> Result<()> {
        // Unregister epoll file to avoid deadlock in file table
        let file = self.files().lock().del(fd)?;

        if let Ok(epoll_file) = file.as_epoll_file() {
            epoll_file.unregister_from_file_table();
        }

        file.release_advisory_locks();
        Ok(())
    }

    /// Close all files in the file table. It will release the POSIX advisory locks owned
    /// by current process.
    pub fn close_all_files(&self) {
        let files = self.files().lock().del_all();
        for file in files {
            if let Ok(epoll_file) = file.as_epoll_file() {
                // Unregister epoll file to avoid deadlock in file table
                epoll_file.unregister_from_file_table();
            }

            file.release_advisory_locks();
        }
    }

    pub fn fs(&self) -> &FsViewRef {
        &self.fs
    }

    pub fn nice(&self) -> &NiceValueRef {
        &self.nice
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

    pub fn robust_list(&self) -> Option<NonNull<RobustListHead>> {
        *self.robust_list.read().unwrap()
    }

    pub fn set_robust_list(&self, new_robust_list: Option<NonNull<RobustListHead>>) {
        *self.robust_list.write().unwrap() = new_robust_list;
    }

    /// Walks the robust futex list, marking futex dead and wake waiters.
    /// It corresponds to Linux's exit_robust_list(), errors are silently ignored.
    pub fn wake_robust_list(&self) {
        let list_head_ptr = match self.robust_list() {
            None => {
                return;
            }
            Some(robust_list) => robust_list.as_ptr(),
        };
        debug!("wake the rubust_list: {:?}", list_head_ptr);
        let robust_list = {
            // Invalid pointer, stop scanning the list further
            if crate::util::mem_util::from_user::check_ptr(list_head_ptr).is_err() {
                return;
            }
            unsafe { &*list_head_ptr }
        };
        for futex_addr in robust_list.futexes() {
            super::do_robust_list::wake_robust_futex(futex_addr, self.tid());
        }
        self.set_robust_list(None);
    }

    pub fn name(&self) -> ThreadName {
        self.name.read().unwrap().clone()
    }

    pub fn set_name(&self, new_name: ThreadName) {
        *self.name.write().unwrap() = new_name;
    }

    pub fn host_eventfd(&self) -> &Arc<HostEventFd> {
        &self.host_eventfd
    }

    pub fn io_buffer(&self) -> UntrustedSliceAllocGuard<'_> {
        self.io_buffer
            .call_once(|| UntrustedSliceAlloc::new(IO_BUF_SIZE).unwrap())
            .guard()
    }

    pub(super) fn start(&self, host_tid: pid_t) {
        self.sched().lock().unwrap().attach(host_tid);
        let mut raw_ptr = self.raw_ptr.write().unwrap();
        *raw_ptr = (unsafe { sgx_thread_get_self() } as usize);

        // Before the thread starts, this thread could be stopped by other threads
        if self.is_forced_to_stop() || self.is_stopped() {
            info!("thread is forced to stopped before this thread starts");
        } else {
            self.inner().start();
        }

        let eventfd = EventFile::new(
            0,
            EventCreationFlags::EFD_CLOEXEC | EventCreationFlags::EFD_NONBLOCK,
        )
        .unwrap();

        let event_file = THREAD_NOTIFIERS.lock().unwrap().insert(self.tid(), eventfd);

        assert!(
            event_file.is_none(),
            "this thread should not have an eventfd before start"
        );

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

        THREAD_NOTIFIERS
            .lock()
            .unwrap()
            .remove(&self.tid())
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

    pub fn force_stop(&self) {
        let mut inner = self.inner();
        // If the thread is not exited or stopped, then notify it to stop
        if inner.status() != ThreadStatus::Exited && inner.status() != ThreadStatus::Stopped {
            inner.notify_stop();
        }
    }

    pub fn is_forced_to_stop(&self) -> bool {
        self.inner().status() == ThreadStatus::ToStop
    }

    pub fn is_stopped(&self) -> bool {
        self.inner().status() == ThreadStatus::Stopped
    }

    pub fn resume(&self) {
        let mut inner = self.inner();
        inner.resume();
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
    ToStop, // notified to stop, not stopped yet
    Stopped,
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
            Self::ToStop { .. } => ThreadStatus::ToStop,
            Self::Stopped { .. } => ThreadStatus::Stopped,
        }
    }

    pub fn term_status(&self) -> Option<TermStatus> {
        match self {
            Self::Exited { term_status } => Some(*term_status),
            _ => None,
        }
    }

    pub fn start(&mut self) {
        *self = Self::Running;
    }

    pub fn notify_stop(&mut self) {
        *self = Self::ToStop;
    }

    pub fn stop(&mut self) {
        *self = Self::Stopped;
    }

    pub fn resume(&mut self) {
        *self = Self::Running;
    }

    pub fn exit(&mut self, term_status: TermStatus) {
        *self = Self::Exited { term_status };
    }
}

extern "C" {
    pub(crate) fn sgx_thread_get_self() -> *const c_void;
}
