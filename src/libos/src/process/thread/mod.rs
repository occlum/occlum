use async_rt::task::{Task, Tirqs};
use async_rt::wait::Waiter;
use std::fmt;
use std::ptr::NonNull;

use super::{
    FileTableRef, ForcedExitStatus, FsViewRef, NiceValueRef, ProcessRef, ProcessVM, ProcessVMRef,
    ResourceLimitsRef, RobustListHead, SchedAgentRef, TermStatus, ThreadRef,
};
use crate::prelude::*;
use crate::signal::{SigQueues, SigSet, SigStack};

pub use self::builder::ThreadBuilder;
pub use self::id::ThreadId;
pub use self::name::ThreadName;

mod builder;
mod id;
mod name;

pub struct Thread {
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
    nice: NiceValueRef,
    rlimits: ResourceLimitsRef,
    // Signal
    sig_queues: RwLock<SigQueues>,
    sig_mask: RwLock<SigSet>,
    sig_stack: SgxMutex<Option<SigStack>>,
    // Waiter
    waiter: Waiter,
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum ThreadStatus {
    Init,
    Running,
    Exited,
    Stopped,
}

impl Thread {
    pub fn process(&self) -> &ProcessRef {
        &self.process
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

    /// Get the signal mask.
    pub fn sig_mask(&self) -> SigSet {
        *self.sig_mask.read().unwrap()
    }

    pub fn waiter(&self) -> &Waiter {
        &self.waiter
    }

    /// Set a new signal mask, returning the old one.
    ///
    /// According to man pages, "it is not possible to block SIGKILL or SIGSTOP.
    /// Attempts to do so are silently ignored." So this method will ignore
    /// SIGKILL and SIGSTOP even if they are given in the new mask.
    ///
    /// This method also updates the TIRQ mask so that the two masks are kept in sync.
    pub fn set_sig_mask(&self, mut new_sig_mask: SigSet) -> SigSet {
        use crate::signal::{SIGKILL, SIGSTOP};
        new_sig_mask -= SIGKILL;
        new_sig_mask -= SIGSTOP;

        let mut sig_mask = self.sig_mask.write().unwrap();
        let old_sig_mask = *sig_mask;
        *sig_mask = new_sig_mask;
        drop(sig_mask);

        // Keep the TIRQ mask in sync with the signal mask
        debug_assert!(self.task().map(|t| t.tid()) == current!().task().map(|t| t.tid()));
        Tirqs::set_mask(new_sig_mask.to_c() as u64);

        old_sig_mask
    }

    /// Get the alternate signal stack.
    pub fn sig_stack(&self) -> &SgxMutex<Option<SigStack>> {
        &self.sig_stack
    }

    /// Get a file from the file table.
    pub fn file(&self, fd: FileDesc) -> Result<FileRef> {
        self.files().lock().unwrap().get(fd)
    }

    /// Add a file to the file table.
    pub fn add_file(&self, new_file: FileRef, close_on_spawn: bool) -> FileDesc {
        self.files().lock().unwrap().put(new_file, close_on_spawn)
    }

    /// Remove a file from the file table.
    pub fn remove_file(&self, fd: FileDesc) -> Result<()> {
        let _file = self.files().lock().unwrap().del(fd)?;
        Ok(())
    }

    /// Remove all files from the file table and close all the files. It will release the POSIX advisory locks owned
    /// by current process.
    pub async fn close_all_files_when_exit(&self) {
        let files = self.files().lock().unwrap().del_all();
        for file in files {
            file.clean_for_close().await;
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
        debug!("wake the robust_list: {:?}", list_head_ptr);
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

    /// Get the task that the thread is executing on.
    pub fn task(&self) -> Option<Arc<Task>> {
        self.sched().lock().unwrap().task()
    }

    pub fn start(&self) {
        self.sched()
            .lock()
            .unwrap()
            .attach(async_rt::task::current::get());
        Tirqs::set_mask(self.sig_mask().to_c() as u64);

        // Before the thread starts, this thread could be stopped by other threads
        if self.is_forced_to_stop() {
            info!("thread is forced to stopped before this thread starts");
        } else {
            self.inner().start();
        }
        /*
                let eventfd = EventFile::new(
                    0,
                    EventCreationFlags::EFD_CLOEXEC | EventCreationFlags::EFD_NONBLOCK,
                )
                .unwrap();

                THREAD_NOTIFIERS
                    .lock()
                    .unwrap()
                    .insert(self.tid(), eventfd)
                    .expect_none("this thread should not have an eventfd before start");
        */
    }

    pub(super) async fn stop(&self) {
        self.waiter.reset();

        while self.is_forced_to_stop() {
            self.waiter.wait().await;
        }
    }

    pub(super) fn wake(&self) {
        self.waiter.waker().wake();
    }

    pub(super) fn exit(&self, term_status: TermStatus) -> usize {
        /*
        THREAD_NOTIFIERS
            .lock()
            .unwrap()
            .remove(&self.tid())
            .unwrap();
        */

        self.sched().lock().unwrap().detach();

        // Remove this thread from its owner process
        let remaining_thread_num = self.remove_thread_from_owner_process();
        self.inner().exit(term_status);

        remaining_thread_num
    }

    // Call this function when a thread exits before the thread is scheduled to run
    pub(super) fn exit_early(&self, term_status: TermStatus) -> usize {
        // Don't call sched detach here. Because this thread has never been scheduled to run,
        warn!("Thread early exit here");
        let remaining_thread_num = self.remove_thread_from_owner_process();
        self.inner().exit_early(term_status);

        remaining_thread_num
    }

    fn remove_thread_from_owner_process(&self) -> usize {
        let mut process_inner = self.process.inner();
        let threads = process_inner.threads_mut().unwrap();
        let thread_i = threads
            .iter()
            .position(|thread| thread.tid() == self.tid())
            .expect("the thread must belong to the process");
        threads.swap_remove(thread_i);
        threads.len()
    }

    pub(super) fn inner(&self) -> SgxMutexGuard<ThreadInner> {
        self.inner.lock().unwrap()
    }

    pub fn force_stop(&self) {
        let mut inner = self.inner();
        inner.stop();
    }

    pub fn is_forced_to_stop(&self) -> bool {
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
// An explicit implementation of Debug trait is required since Process and Thread
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
    Running,
    Exited { term_status: TermStatus },
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

    pub fn stop(&mut self) {
        *self = Self::Stopped;
    }

    pub fn resume(&mut self) {
        *self = Self::Running;
    }

    pub fn exit(&mut self, term_status: TermStatus) {
        debug_assert!(self.status() == ThreadStatus::Running);
        *self = Self::Exited { term_status };
    }

    pub fn exit_early(&mut self, term_status: TermStatus) {
        *self = Self::Exited { term_status };
    }
}
