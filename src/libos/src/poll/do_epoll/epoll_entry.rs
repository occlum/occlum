use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::sync::Weak;

use new_self_ref_arc::new_self_ref_arc;

use super::{EpollEvent, EpollFile, EpollFlags};
use crate::fs::{Events, WeakFileRef};
use crate::prelude::*;

/// An epoll entry contained in an epoll file. Each epoll entry is added, modified,
/// or deleted by the `EpollCtl` command.
#[derive(Debug)]
pub struct EpollEntry {
    fd: FileDesc,
    file: WeakFileRef,
    inner: SgxMutex<Inner>,
    // Whether the entry is in the ready list
    is_ready: AtomicBool,
    // Whether the entry has been deleted from the interest list
    is_deleted: AtomicBool,
    // Refers to the epoll file containing this epoll entry
    weak_epoll: Weak<EpollFile>,
    // An EpollEntry is always contained inside Arc
    weak_self: Weak<EpollEntry>,
}

#[derive(Debug)]
struct Inner {
    event: EpollEvent,
    flags: EpollFlags,
}

impl EpollEntry {
    /// Creates a new epoll entry associated with the given epoll file.
    ///
    /// An `EpollEntry` is always contained inside `Arc`.
    pub fn new(
        fd: FileDesc,
        file: WeakFileRef,
        event: EpollEvent,
        flags: EpollFlags,
        weak_epoll: Weak<EpollFile>,
    ) -> Arc<Self> {
        let new_self = Self {
            fd,
            file,
            inner: SgxMutex::new(Inner { event, flags }),
            is_ready: AtomicBool::new(false),
            is_deleted: AtomicBool::new(false),
            weak_epoll,
            weak_self: Weak::new(),
        };
        new_self_ref_arc!(new_self)
    }

    /// Get the epoll file associated with this epoll entry.
    pub fn epoll_file(&self) -> Option<Arc<EpollFile>> {
        self.weak_epoll.upgrade()
    }

    /// Get an instance of `Arc` that refers to this epoll entry.
    pub fn self_arc(&self) -> Arc<Self> {
        self.weak_self.upgrade().unwrap()
    }

    /// Get the file associated with this epoll entry.
    ///
    /// Since an epoll entry only holds a weak reference to the file,
    /// it is possible (albeit unlikely) that the file has been dropped.
    pub fn file(&self) -> Option<FileRef> {
        self.file.upgrade()
    }

    /// Get the epoll event associated with the epoll entry.
    pub fn event(&self) -> EpollEvent {
        let inner = self.inner.lock().unwrap();
        inner.event
    }

    /// Get the epoll flags associated with the epoll entry.
    pub fn flags(&self) -> EpollFlags {
        let inner = self.inner.lock().unwrap();
        inner.flags
    }

    /// Get the epoll event and flags that are associated with this epoll entry.
    pub fn event_and_flags(&self) -> (EpollEvent, EpollFlags) {
        let inner = self.inner.lock().unwrap();
        (inner.event, inner.flags)
    }

    /// Poll the events of the file associated with this epoll entry.
    ///
    /// If the returned events is not empty, then the file is considered ready.
    pub fn poll(&self) -> Events {
        match self.file.upgrade() {
            Some(file) => file.poll(Events::all(), None),
            None => Events::empty(),
        }
    }

    /// Update the epoll entry, most likely to be triggered via `EpollCtl::Mod`.
    pub fn update(&self, event: EpollEvent, flags: EpollFlags) {
        let mut inner = self.inner.lock().unwrap();
        *inner = Inner { event, flags }
    }

    /// Returns whether the epoll entry is in the ready list.
    pub fn is_ready(&self) -> bool {
        self.is_ready.load(Relaxed)
    }

    /// Mark the epoll entry as being in the ready list.
    pub fn set_ready(&self) {
        self.is_ready.store(true, Relaxed);
    }

    /// Mark the epoll entry as not being in the ready list.
    pub fn reset_ready(&self) {
        self.is_ready.store(false, Relaxed)
    }

    /// Returns whether the epoll entry has been deleted from the interest list.
    pub fn is_deleted(&self) -> bool {
        self.is_deleted.load(Relaxed)
    }

    /// Mark the epoll entry as having been deleted from the interest list.
    pub fn set_deleted(&self) {
        self.is_deleted.store(true, Relaxed);
    }

    /// Get the file descriptor associated with the epoll entry.
    pub fn fd(&self) -> FileDesc {
        self.fd
    }
}
