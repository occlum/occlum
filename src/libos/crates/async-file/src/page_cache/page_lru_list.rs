#[cfg(feature = "sgx")]
use std::prelude::v1::*;
use std::sync::Arc;

use crate::page_cache::{PageEntry, PageEntryInner};
use crate::util::lru_list::{LruEntry, LruList};

pub struct PageLruList {
    inner: LruList<PageEntryInner>,
}

impl PageLruList {
    pub fn new() -> Self {
        let inner = LruList::new();
        Self { inner }
    }

    pub fn insert(&mut self, entry: PageEntry) {
        let entry_inner = entry.unwrap();
        self.inner.insert(entry_inner)
    }

    pub fn touch(&mut self, entry: &PageEntry) {
        self.inner.touch(entry.inner())
    }

    pub fn remove(&mut self, entry: &PageEntry) {
        self.inner.remove(entry.inner())
    }

    pub fn evict(&mut self) -> Option<PageEntry> {
        self.inner.evict().map(|inner| PageEntry::wrap(inner))
    }

    pub fn evict_nr(&mut self, max_count: usize) -> Vec<PageEntry> {
        let evicted: Vec<Arc<LruEntry<PageEntryInner>>> = self.inner.evict_nr(max_count);
        unsafe {
            // This transmute is ok beceause PageEntry has the same memory layout as
            // Arc<LruEntry<PageEntryInner>>.
            std::mem::transmute(evicted)
        }
    }

    pub fn evict_nr_with(
        &mut self,
        max_count: usize,
        cond: impl FnMut(&PageEntryInner) -> bool,
    ) -> Vec<PageEntry> {
        let evicted: Vec<Arc<LruEntry<PageEntryInner>>> = self.inner.evict_nr_with(max_count, cond);
        unsafe {
            // This transmute is ok beceause PageEntry has the same memory layout as
            // Arc<LruEntry<PageEntryInner>>.
            std::mem::transmute(evicted)
        }
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }
}

impl Default for PageLruList {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for PageLruList {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list()
            .entries(self.inner.iter().map(|lru_entry| lru_entry.inner()))
            .finish()
    }
}
