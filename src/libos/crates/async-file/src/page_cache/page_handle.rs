use std::any::Any;
#[cfg(feature = "sgx")]
use std::prelude::v1::*;
#[cfg(not(feature = "sgx"))]
use std::sync::{Arc, MutexGuard};
#[cfg(feature = "sgx")]
use std::sync::{Arc, SgxMutexGuard as MutexGuard};

use crate::page_cache::{Page, PageEntry, PageState};

/// Page handle is the user's view of page entry.
///
/// Memory safety. It is critical that the representation is transparent.
/// We rely on this property for zero-overhead type conversion between PageHandle
/// and PageEntry.
#[repr(transparent)]
pub struct PageHandle(PageEntry);

impl PageHandle {
    pub fn wrap(entry: PageEntry) -> Self {
        Self(entry)
    }

    pub(crate) fn unwrap(self) -> PageEntry {
        unsafe { std::mem::transmute(self) }
    }

    pub fn file(&self) -> &Arc<dyn Any + Send + Sync> {
        self.0.file()
    }

    pub fn fd(&self) -> i32 {
        self.0.fd()
    }

    pub fn offset(&self) -> usize {
        self.0.offset()
    }

    pub fn key(&self) -> (i32, usize) {
        self.0.key()
    }

    pub(crate) fn state(&self) -> MutexGuard<PageState> {
        self.0.state()
    }

    pub fn page(&self) -> &Page {
        self.0.page()
    }
}

impl Clone for PageHandle {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl Drop for PageHandle {
    fn drop(&mut self) {
        panic!("PageHandle must be released through PageCache::release()");
    }
}
