use std::any::Any;
use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::mem::{self, MaybeUninit};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Weak;
use std::time::Duration;

use atomic::Atomic;

use super::epoll_waiter::EpollWaiter;
use super::host_file_epoller::HostFileEpoller;
use super::{EpollCtl, EpollEvent, EpollFlags};
use crate::events::{Observer, Waiter, WaiterQueue};
use crate::fs::{
    AtomicIoEvents, File, FileTableEvent, FileTableNotifier, HostFd, IoEvents, IoNotifier,
};
use crate::prelude::*;

// TODO: Prevent two epoll files from monitoring each other, which may cause
// deadlock in the current implementation.
// TODO: Fix unreliable EpollFiles after process spawning. EpollFile is connected
// with the current process's file table by regitering itself as an observer
// to the file table. But if an EpollFile is cloned or inherited by a child
// process, then this EpollFile still has connection with the parent process's
// file table, which is problematic.

/// A file that provides epoll API.
///
/// Conceptually, we maintain two lists: one consists of all interesting files,
/// which can be managed by the epoll ctl commands; the other are for ready files,
/// which are files that have some events. A epoll wait only needs to iterate the
/// ready list and poll each file to see if the file is ready for the interesting
/// I/O.
///
/// To maintain the ready list, we need to monitor interesting events that happen
/// on the files. To do so, the `EpollFile` registers itself as an `Observer` to
/// the `IoNotifier`s of the monotored files. Thus, we can add a file to the ready
/// list when an event happens on the file.
///
/// LibOS files are easy to monitor. LibOS files are implemented by us. We know
/// exactly when an event happens and thus can broadcast it using `IoNotifier`.
///
/// Unlike LibOS files, host files are implemented by the host OS. We have no way
/// to let the host OS _push_ events to us. Luckily, we can do the reverse: _poll_
/// host files to check events. And there is a good timing for it; that is, at
/// every epoll wait call. We have made a helper called `HostFileEpoller`, which can
/// poll events on a set of host files and trigger their associated `Notifier`s to
/// broadcast their events, e.g., to `EpollFile`.
///
/// This way, both LibOS files and host files can notify the `EpollFile` about
/// their events.
pub struct EpollFile {
    // All interesting entries.
    interest: SgxMutex<HashMap<FileDesc, Arc<EpollEntry>>>,
    // Entries that are probably ready (having events happened).
    ready: SgxMutex<VecDeque<Arc<EpollEntry>>>,
    // All threads that are waiting on this epoll file.
    waiters: WaiterQueue,
    // A notifier to broadcast events on this epoll file.
    notifier: IoNotifier,
    // A helper to poll the events on the interesting host files.
    host_file_epoller: HostFileEpoller,
    // Any EpollFile is wrapped with Arc when created.
    weak_self: Weak<Self>,
    // Host events
    host_events: Atomic<IoEvents>,
}

impl EpollFile {
    pub fn new() -> Arc<Self> {
        let interest = Default::default();
        let ready = Default::default();
        let waiters = WaiterQueue::new();
        let notifier = IoNotifier::new();
        let host_file_epoller = HostFileEpoller::new();
        let weak_self = Default::default();
        let host_events = Atomic::new(IoEvents::empty());

        let arc_self = Self {
            interest,
            ready,
            waiters,
            notifier,
            host_file_epoller,
            weak_self,
            host_events,
        }
        .wrap_self();

        arc_self.register_to_file_table();
        arc_self
    }

    fn wrap_self(self) -> Arc<Self> {
        let mut strong_self = Arc::new(self);
        let weak_self = Arc::downgrade(&strong_self);

        unsafe {
            let ptr_self = Arc::into_raw(strong_self) as *mut Self;
            (*ptr_self).weak_self = weak_self;
            strong_self = Arc::from_raw(ptr_self);
        }

        strong_self
    }

    fn register_to_file_table(&self) {
        let weak_observer = self.weak_self.clone() as Weak<dyn Observer<_>>;
        let thread = current!();
        let file_table = thread.files().lock().unwrap();
        file_table.notifier().register(weak_observer, None, None);
    }

    fn unregister_from_file_table(&self) {
        let weak_observer = self.weak_self.clone() as Weak<dyn Observer<_>>;
        let thread = current!();
        let file_table = thread.files().lock().unwrap();
        file_table.notifier().unregister(&weak_observer);
    }

    pub fn control(&self, cmd: &EpollCtl) -> Result<()> {
        debug!("epoll control: cmd = {:?}", cmd);

        match cmd {
            EpollCtl::Add(fd, event, flags) => {
                self.add_interest(*fd, *event, *flags)?;
            }
            EpollCtl::Del(fd) => {
                self.del_interest(*fd)?;
            }
            EpollCtl::Mod(fd, event, flags) => {
                self.mod_interest(*fd, *event, *flags)?;
            }
        }
        Ok(())
    }

    pub fn wait(
        &self,
        revents: &mut [MaybeUninit<EpollEvent>],
        timeout: Option<&Duration>,
    ) -> Result<usize> {
        debug!("epoll wait: timeout = {:?}", timeout);

        let mut timeout = timeout.cloned();
        let max_count = revents.len();
        let mut reinsert = VecDeque::with_capacity(max_count);
        let waiter = EpollWaiter::new(&self.host_file_epoller);

        loop {
            // Poll the latest states of the interested host files. If a host
            // file is ready, then it will be pushed into the ready list. Note
            // that this is the only way through which a host file can appear in
            // the ready list. This ensures that only the host files whose
            // events are update-to-date will be returned, reducing the chances
            // of false positive results to the minimum.
            self.host_file_epoller.poll_events(max_count);

            // Prepare for the waiter.wait_mut() at the end of the loop
            self.waiters.reset_and_enqueue(waiter.as_ref());

            // Pop from the ready list to find as many results as possible
            let mut count = 0;
            while count < max_count {
                // Pop some entries from the ready list
                let mut ready_entries = self.pop_ready(max_count - count);
                if ready_entries.len() == 0 {
                    break;
                }

                // Note that while iterating the ready entries, we do not hold the lock
                // of the ready list. This reduces the chances of lock contention.
                for ep_entry in ready_entries.into_iter() {
                    if ep_entry.is_deleted.load(Ordering::Acquire) {
                        continue;
                    }

                    // Poll the file that corresponds to the entry
                    let mut inner = ep_entry.inner.lock().unwrap();
                    let mask = inner.event.mask();
                    let file = &ep_entry.file;
                    let events = file.poll_new() & mask;
                    if events.is_empty() {
                        continue;
                    }

                    // We find a ready file!
                    let mut revent = inner.event;
                    revent.mask = events;
                    revents[count].write(revent);
                    count += 1;

                    // Behave differently according the epoll flags

                    if inner.flags.contains(EpollFlags::ONE_SHOT) {
                        inner.event.mask = IoEvents::empty();
                    }

                    if !inner
                        .flags
                        .intersects(EpollFlags::EDGE_TRIGGER | EpollFlags::ONE_SHOT)
                    {
                        drop(inner);

                        // Host files should not be reinserted into the ready list
                        if ep_entry.file.host_fd().is_none() {
                            reinsert.push_back(ep_entry);
                        }
                    }
                }
            }

            // If any results, we can return
            if count > 0 {
                // Push the entries that are still ready after polling back to the ready list
                if reinsert.len() > 0 {
                    self.push_ready_iter(reinsert.into_iter());
                }

                return Ok(count);
            }

            // Wait for a while to try again later.
            let ret = waiter.wait_mut(timeout.as_mut());
            if let Err(e) = ret {
                if e.errno() == ETIMEDOUT {
                    return Ok(0);
                } else {
                    return Err(e);
                }
            }
            // This means we have been waken up successfully. Let's try again.
        }
    }

    fn add_interest(&self, fd: FileDesc, mut event: EpollEvent, flags: EpollFlags) -> Result<()> {
        let file = current!().file(fd)?;

        let arc_self = self.weak_self.upgrade().unwrap();
        if Arc::ptr_eq(&(arc_self as Arc<dyn File>), &file) {
            return_errno!(EINVAL, "a epoll file cannot epoll itself");
        }

        self.check_flags(&flags);
        self.prepare_event(&mut event);

        let ep_entry = Arc::new(EpollEntry::new(fd, file, event, flags));

        // A critical section protected by the lock of self.interest
        {
            let notifier = ep_entry
                .file
                .notifier()
                .ok_or_else(|| errno!(EINVAL, "a file must has an associated notifier"))?;

            let mut interest_entries = self.interest.lock().unwrap();
            if interest_entries.get(&fd).is_some() {
                return_errno!(EEXIST, "fd is already registered");
            }
            interest_entries.insert(fd, ep_entry.clone());

            // Start observing events on the target file.
            let weak_observer = self.weak_self.clone() as Weak<dyn Observer<_>>;
            let weak_ep_entry = Arc::downgrade(&ep_entry);
            notifier.register(weak_observer, Some(IoEvents::all()), Some(weak_ep_entry));

            // Handle host file
            if ep_entry.file.host_fd().is_some() {
                self.host_file_epoller
                    .add_file(ep_entry.file.clone(), event, flags);
                return Ok(());
            }
        }

        self.push_ready(ep_entry);

        Ok(())
    }

    fn del_interest(&self, fd: FileDesc) -> Result<()> {
        // A critical section protected by the lock of self.interest
        {
            let mut interest_entries = self.interest.lock().unwrap();
            let ep_entry = interest_entries
                .remove(&fd)
                .ok_or_else(|| errno!(ENOENT, "fd is not added"))?;
            ep_entry.is_deleted.store(true, Ordering::Release);

            let notifier = ep_entry.file.notifier().unwrap();
            let weak_observer = self.weak_self.clone() as Weak<dyn Observer<_>>;
            notifier.unregister(&weak_observer);

            if ep_entry.file.host_fd().is_some() {
                self.host_file_epoller.del_file(&ep_entry.file);
            }
        }
        Ok(())
    }

    fn mod_interest(&self, fd: FileDesc, mut event: EpollEvent, flags: EpollFlags) -> Result<()> {
        self.check_flags(&flags);
        self.prepare_event(&mut event);

        // A critical section protected by the lock of self.interest
        let ep_entry = {
            let mut interest_entries = self.interest.lock().unwrap();
            let ep_entry = interest_entries
                .get(&fd)
                .ok_or_else(|| errno!(ENOENT, "fd is not added"))?
                .clone();

            let new_ep_inner = EpollEntryInner { event, flags };
            let mut old_ep_inner = ep_entry.inner.lock().unwrap();
            if *old_ep_inner == new_ep_inner {
                return Ok(());
            }
            *old_ep_inner = new_ep_inner;
            drop(old_ep_inner);

            if ep_entry.file.host_fd().is_some() {
                self.host_file_epoller
                    .mod_file(&ep_entry.file, event, flags);
                return Ok(());
            }

            ep_entry
        };

        self.push_ready(ep_entry);

        Ok(())
    }

    fn push_ready(&self, ep_entry: Arc<EpollEntry>) {
        // Fast path to avoid locking
        if ep_entry.is_ready.load(Ordering::Relaxed) {
            // Concurrency note:
            // What if right after returning a true value of `is_ready`, then the `EpollEntry` is
            // popped from the ready list? Does it mean than we miss an interesting event?
            //
            // The answer is NO. If the `is_ready` field of an `EpollEntry` turns from `true` to
            // `false`, then the `EpollEntry` must be popped out of the ready list and its
            // corresponding file must be polled in the `wait` method. This means that we have
            // taken into account any interesting events happened on the file so far.
            return;
        }

        self.push_ready_iter(std::iter::once(ep_entry));
    }

    fn push_ready_iter<I: Iterator<Item = Arc<EpollEntry>>>(&self, ep_entries: I) {
        let mut has_pushed_any = false;

        // A critical section protected by self.ready.lock()
        {
            let mut ready_entries = self.ready.lock().unwrap();
            for ep_entry in ep_entries {
                if ep_entry.is_ready.load(Ordering::Relaxed) {
                    continue;
                }

                ep_entry.is_ready.store(true, Ordering::Relaxed);
                ready_entries.push_back(ep_entry);

                has_pushed_any = true;
            }
        }

        if has_pushed_any {
            self.mark_ready();
        }
    }

    fn pop_ready(&self, max_count: usize) -> VecDeque<Arc<EpollEntry>> {
        // A critical section protected by self.ready.lock()
        {
            let mut ready_entries = self.ready.lock().unwrap();
            let max_count = max_count.min(ready_entries.len());
            ready_entries
                .drain(..max_count)
                .map(|ep_entry| {
                    ep_entry.is_ready.store(false, Ordering::Relaxed);
                    ep_entry
                })
                .collect::<VecDeque<Arc<EpollEntry>>>()
        }
    }

    fn mark_ready(&self) {
        self.notifier.broadcast(&IoEvents::IN);
        self.waiters.dequeue_and_wake_all();
    }

    fn check_flags(&self, flags: &EpollFlags) {
        if flags.intersects(EpollFlags::EXCLUSIVE | EpollFlags::WAKE_UP) {
            warn!("{:?} contains unsupported flags", flags);
        }
    }

    fn prepare_event(&self, event: &mut EpollEvent) {
        // Add two events that are reported by default
        event.mask |= (IoEvents::ERR | IoEvents::HUP);
    }
}

impl File for EpollFile {
    fn poll_new(&self) -> IoEvents {
        if self
            .host_events
            .load(Ordering::Acquire)
            .contains(IoEvents::IN)
        {
            return IoEvents::IN;
        }

        let ready_entries = self.ready.lock().unwrap();
        if !ready_entries.is_empty() {
            return IoEvents::IN;
        }

        IoEvents::empty()
    }

    fn notifier(&self) -> Option<&IoNotifier> {
        Some(&self.notifier)
    }

    fn host_fd(&self) -> Option<&HostFd> {
        Some(self.host_file_epoller.host_fd())
    }

    fn update_host_events(&self, ready: &IoEvents, mask: &IoEvents, trigger_notifier: bool) {
        self.host_events.update(ready, mask, Ordering::Release);

        if trigger_notifier {
            self.notifier.broadcast(ready);
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Drop for EpollFile {
    fn drop(&mut self) {
        // Do not try to `self.weak_self.upgrade()`! The Arc object must have been
        // dropped at this point.
        let self_observer = self.weak_self.clone() as Weak<dyn Observer<IoEvents>>;

        // Unregister ourself from all interesting files' notifiers
        let mut interest_entries = self.interest.lock().unwrap();
        interest_entries.drain().for_each(|(_, ep_entry)| {
            if let Some(notifier) = ep_entry.file.notifier() {
                notifier.unregister(&self_observer);
            }
        });

        self.unregister_from_file_table();
    }
}

impl Observer<IoEvents> for EpollFile {
    fn on_event(&self, _events: &IoEvents, metadata: &Option<Weak<dyn Any + Send + Sync>>) {
        let ep_entry_opt = metadata
            .as_ref()
            .and_then(|weak_any| weak_any.upgrade())
            .and_then(|strong_any| strong_any.downcast().ok());
        let ep_entry: Arc<EpollEntry> = match ep_entry_opt {
            None => return,
            Some(ep_entry) => ep_entry,
        };

        self.push_ready(ep_entry);
    }
}

impl Observer<FileTableEvent> for EpollFile {
    fn on_event(&self, event: &FileTableEvent, _metadata: &Option<Weak<dyn Any + Send + Sync>>) {
        let FileTableEvent::Del(fd) = event;
        let _ = self.del_interest(*fd);
    }
}

impl fmt::Debug for EpollFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EpollFile")
            .field("interest", &self.interest.lock().unwrap())
            .field("ready", &self.ready.lock().unwrap())
            .finish()
    }
}

pub trait AsEpollFile {
    fn as_epoll_file(&self) -> Result<&EpollFile>;
}

impl AsEpollFile for FileRef {
    fn as_epoll_file(&self) -> Result<&EpollFile> {
        self.as_any()
            .downcast_ref::<EpollFile>()
            .ok_or_else(|| errno!(EBADF, "not an epoll file"))
    }
}

#[derive(Debug)]
struct EpollEntry {
    fd: FileDesc,
    file: FileRef,
    inner: SgxMutex<EpollEntryInner>,
    // Whether the entry is in the ready list
    is_ready: AtomicBool,
    // Whether the entry has been deleted from the interest list
    is_deleted: AtomicBool,
}

impl EpollEntry {
    pub fn new(fd: FileDesc, file: FileRef, event: EpollEvent, flags: EpollFlags) -> Self {
        let is_ready = Default::default();
        let is_deleted = Default::default();
        let inner = SgxMutex::new(EpollEntryInner { event, flags });
        Self {
            fd,
            file,
            inner,
            is_ready,
            is_deleted,
        }
    }
}

#[derive(Debug, PartialEq)]
struct EpollEntryInner {
    event: EpollEvent,
    flags: EpollFlags,
}
