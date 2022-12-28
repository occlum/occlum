use crate::fs::Inode;
use crate::metadata::InodeId;
use crate::prelude::*;

use lru::LruCache;
use std::fmt::{Debug, Formatter};
use std::mem::size_of_val;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

/// Convert struct to bytes slice
pub unsafe trait AsBuf {
    fn as_buf(&self) -> &[u8] {
        // Form the slice with byte length of the pointed value
        unsafe { std::slice::from_raw_parts(self as *const _ as *const u8, size_of_val(self)) }
    }
    fn as_buf_mut(&mut self) -> &mut [u8] {
        // Form the slice with byte length of the pointed value
        unsafe { std::slice::from_raw_parts_mut(self as *mut _ as *mut u8, size_of_val(self)) }
    }
}

/// Dirty wraps a value of type T with functions similar to that of a Read/Write
/// lock but simply sets a dirty flag on write(), reset on read()
pub struct Dirty<T: Clone + Debug> {
    value: T,
    dirty: bool,
}

impl<T: Clone + Debug> Dirty<T> {
    /// Create a new Dirty
    pub fn new(val: T) -> Dirty<T> {
        Dirty {
            value: val,
            dirty: false,
        }
    }

    /// Create a new Dirty with dirty set
    pub fn new_dirty(val: T) -> Dirty<T> {
        Dirty {
            value: val,
            dirty: true,
        }
    }

    /// Returns true if dirty, false otherwise
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Reset dirty
    pub fn sync(&mut self) {
        self.dirty = false;
    }
}

impl<T: Clone + Debug> Deref for Dirty<T> {
    type Target = T;

    /// Read the value
    fn deref(&self) -> &T {
        &self.value
    }
}

impl<T: Clone + Debug> DerefMut for Dirty<T> {
    /// Writable value return, sets the dirty flag
    fn deref_mut(&mut self) -> &mut T {
        self.dirty = true;
        &mut self.value
    }
}

impl<T: Clone + Debug> Drop for Dirty<T> {
    /// Guard it is not dirty when dropping
    fn drop(&mut self) {
        if self.is_dirty() {
            warn!("[{:?}] is dirty then dropping", self);
        }
    }
}

impl<T: Clone + Debug> Debug for Dirty<T> {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        let tag = if self.dirty { "Dirty" } else { "Clean" };
        write!(f, "[{}] {:?}", tag, self.value)
    }
}

/// A LRU Cache to hold inodes
pub struct InodeCache {
    inner: LruCache<InodeId, Arc<Inode>>,
    soft_cap: usize,
}

impl InodeCache {
    /// Creates a new LRU Cache that holds at most soft capacity items.
    pub fn new(soft_cap: usize) -> Self {
        Self {
            inner: LruCache::unbounded(),
            soft_cap,
        }
    }

    /// Pushes a (id, inode) pair into the cache.
    /// If an entry with id already exists in the cache or
    /// another cache entry is removed (due to the lru’s policy),
    /// then it returns the old entry’s (id, inode) pair.
    pub fn push(&mut self, k: InodeId, v: Arc<Inode>) -> Option<(InodeId, Arc<Inode>)> {
        if self.inner.len() < self.soft_cap {
            return self.inner.push(k, v);
        }

        // cache is full
        for _ in 0..self.soft_cap {
            let (id, inode) = self.inner.pop_lru().unwrap();
            // cache is the last owner of the inode
            if Arc::strong_count(&inode) == 1 {
                self.inner.push(k, v);
                return Some((id, inode));
            }
            self.inner.put(id, inode);
        }

        // cannot pop item, expand the capacity of cache
        self.soft_cap *= 2;
        self.inner.push(k, v)
    }

    /// Returns a reference to the value of the key in the cache
    /// or None if it is not present in the cache.
    /// Moves the key to the head of the LRU list if it exists.
    pub fn get(&mut self, k: &InodeId) -> Option<&Arc<Inode>> {
        self.inner.get(k)
    }

    /// Removes and returns the value corresponding to the key
    /// from the cache or None if it does not exist.
    pub fn pop(&mut self, k: &InodeId) -> Option<Arc<Inode>> {
        self.inner.pop(k)
    }

    /// Retains only the elements specified by the predicate,
    /// it returns all the values in cache.
    pub fn retain_items<F>(&mut self, mut f: F) -> Vec<Arc<Inode>>
    where
        F: FnMut(&Arc<Inode>) -> bool,
    {
        let mut values = Vec::with_capacity(self.inner.len());
        let cache_size = self.inner.len();
        for _ in 0..cache_size {
            let (id, inode) = self.inner.pop_lru().unwrap();
            if f(&inode) {
                self.inner.put(id, inode.clone());
            }
            values.push(inode);
        }
        values
    }
}
