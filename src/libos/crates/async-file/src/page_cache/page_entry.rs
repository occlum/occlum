use std::any::Any;
#[cfg(feature = "sgx")]
use std::prelude::v1::*;
#[cfg(not(feature = "sgx"))]
use std::sync::{Arc, Mutex, MutexGuard};
#[cfg(feature = "sgx")]
use std::sync::{Arc, SgxMutex as Mutex, SgxMutexGuard as MutexGuard};

use atomic::{Atomic, Ordering};

use super::LruListName;
use crate::page_cache::{AsFd, Page, PageState};
use crate::util::lru_list::LruEntry;

/// A page entry represents a cache page in the page cache.
///
/// Memory safety. It is important that this type has the identical memory layout
/// with Arc<LruEntry<PageEntryInner>>.
#[repr(transparent)]
pub struct PageEntry(Arc<LruEntry<PageEntryInner>>);

pub struct PageEntryInner {
    file: Arc<dyn Any + Send + Sync>,
    fd: i32,
    offset: usize,
    state: Mutex<PageState>,
    list_name: Atomic<Option<LruListName>>,
    page: Page,
}

// Implementationn for PageEntry

impl PageEntry {
    pub fn new<F>(file: Arc<F>, offset: usize) -> Self
    where
        F: AsFd + Send + Sync + 'static,
    {
        let inner = PageEntryInner::new(file, offset);
        let new_self = Self(Arc::new(LruEntry::new(inner)));
        new_self
    }

    pub fn wrap(inner: Arc<LruEntry<PageEntryInner>>) -> Self {
        Self(inner)
    }

    pub fn unwrap(self) -> Arc<LruEntry<PageEntryInner>> {
        let Self(inner) = self;
        inner
    }

    pub fn key(&self) -> (i32, usize) {
        (self.fd(), self.offset())
    }

    pub unsafe fn reset<F>(&mut self, file: Arc<F>, offset: usize)
    where
        F: AsFd + Send + Sync + 'static,
    {
        debug_assert!(Arc::strong_count(&self.0) == 1);
        debug_assert!(Arc::weak_count(&self.0) == 0);

        let inner_mut = Arc::get_mut_unchecked(&mut self.0);
        inner_mut.inner_mut().fd = file.as_fd();
        inner_mut.inner_mut().file = file as Arc<dyn Any + Send + Sync>;
        inner_mut.inner_mut().offset = offset;
    }

    pub fn file(&self) -> &Arc<dyn Any + Send + Sync> {
        &self.0.inner().file
    }

    pub fn fd(&self) -> i32 {
        self.0.inner().fd
    }

    pub fn offset(&self) -> usize {
        self.0.inner().offset
    }

    pub fn state(&self) -> MutexGuard<PageState> {
        self.0.inner().state.lock().unwrap()
    }

    pub fn page(&self) -> &Page {
        &self.0.inner().page
    }

    pub(super) fn list_name(&self) -> Option<LruListName> {
        self.0.inner().list_name.load(Ordering::Relaxed)
    }

    pub(super) fn set_list_name(&self, new_list_name: Option<LruListName>) {
        self.0
            .inner()
            .list_name
            .store(new_list_name, Ordering::Relaxed)
    }

    pub fn refcnt(this: &Self) -> usize {
        Arc::strong_count(&this.0)
    }

    pub fn inner(&self) -> &Arc<LruEntry<PageEntryInner>> {
        &self.0
    }
}

impl Clone for PageEntry {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl std::fmt::Debug for PageEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PageEntry")
            .field("fd", &self.fd())
            .field("offset", &self.offset())
            .field("state", &*self.state())
            .field("list_name", &self.list_name())
            .finish()
    }
}

// Implementationn for PageEntryInner

impl PageEntryInner {
    pub fn new<F>(file: Arc<F>, offset: usize) -> Self
    where
        F: AsFd + Send + Sync + 'static,
    {
        debug_assert!(offset % Page::size() == 0);
        let fd = file.as_fd();
        let file = file as Arc<dyn Any + Send + Sync>;
        Self {
            file,
            fd,
            offset,
            state: Mutex::new(PageState::Uninit),
            list_name: Atomic::new(None),
            page: Page::new(),
        }
    }

    pub fn fd(&self) -> i32 {
        self.fd
    }
}

impl std::fmt::Debug for PageEntryInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PageEntryInner")
            .field("fd", &self.fd)
            .field("offset", &self.offset)
            .field("state", &*self.state.lock().unwrap())
            .field("list_name", &self.list_name.load(Ordering::Relaxed))
            .finish()
    }
}
