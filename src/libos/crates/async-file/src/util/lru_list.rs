use std::fmt::Debug;
#[cfg(feature = "sgx")]
use std::prelude::v1::*;
use std::sync::Arc;

use atomic::{Atomic, Ordering};
use intrusive_collections::intrusive_adapter;
use intrusive_collections::linked_list::Iter;
use intrusive_collections::{LinkedList, LinkedListLink};

use crate::util::object_id::ObjectId;

pub struct LruList<T> {
    list_id: ObjectId,
    len: usize,
    inner: LinkedList<LinkedListAdapter<T>>,
}

pub struct LruEntry<T> {
    inner: T,
    list_id: Atomic<ObjectId>,
    link: LinkedListLink,
}

intrusive_adapter!(pub LinkedListAdapter<T> =
    Arc<LruEntry<T>>: LruEntry<T> {
        link: LinkedListLink
    }
);

impl<T> LruList<T> {
    pub fn new() -> Self {
        let list_id = ObjectId::new();
        let len = 0;
        // The front is the most recently used, while the back is the least recently used
        let inner = LinkedList::new(LinkedListAdapter::new());
        Self {
            list_id,
            len,
            inner,
        }
    }

    pub fn contains(&self, entry: &Arc<LruEntry<T>>) -> bool {
        entry.list_id.load(Ordering::Acquire) == self.list_id && entry.link.is_linked()
    }

    pub fn insert(&mut self, entry: Arc<LruEntry<T>>) {
        assert!(entry.list_id.swap(self.list_id, Ordering::Relaxed) == ObjectId::null());
        self.inner.push_front(entry);
        self.len += 1;
    }

    pub fn touch(&mut self, entry: &Arc<LruEntry<T>>) {
        let entry = self.do_remove(entry);
        self.insert(entry);
    }

    pub fn remove(&mut self, entry: &Arc<LruEntry<T>>) {
        assert!(entry.list_id.load(Ordering::Relaxed) == self.list_id);
        self.do_remove(entry);
    }

    pub fn evict(&mut self) -> Option<Arc<LruEntry<T>>> {
        let ret = self.inner.pop_back();
        if let Some(entry) = &ret {
            let old_id = entry.list_id.swap(ObjectId::null(), Ordering::Relaxed);
            debug_assert!(old_id == self.list_id);
            self.len -= 1;
        }
        ret
    }

    pub fn evict_nr(&mut self, max_count: usize) -> Vec<Arc<LruEntry<T>>> {
        let mut result = Vec::with_capacity(max_count);
        while result.len() < max_count {
            let entry = match self.inner.pop_back() {
                Some(entry) => entry,
                None => {
                    break;
                }
            };
            let old_id = entry.list_id.swap(ObjectId::null(), Ordering::Relaxed);
            debug_assert!(old_id == self.list_id);
            result.push(entry);
        }
        self.len -= result.len();
        result
    }

    /// Evict all entries that satisfy a given closure.
    pub fn evict_nr_with(
        &mut self,
        max_count: usize,
        mut f: impl FnMut(&T) -> bool,
    ) -> Vec<Arc<LruEntry<T>>> {
        let mut res = Vec::new();
        if max_count == 0 {
            return res;
        }

        let mut cursor = self.inner.back_mut();
        loop {
            let entry = match cursor.get() {
                Some(entry) => entry,
                None => {
                    break;
                }
            };
            let should_evict = f(entry.inner());
            if should_evict {
                let entry = cursor.remove().unwrap();
                let old_id = entry.list_id.swap(ObjectId::null(), Ordering::Relaxed);
                debug_assert!(old_id == self.list_id);
                res.push(entry);
                if res.len() >= max_count {
                    break;
                }
            }
            cursor.move_prev();
        }
        res
    }

    fn do_remove(&mut self, entry: &Arc<LruEntry<T>>) -> Arc<LruEntry<T>> {
        assert!(entry.list_id.swap(ObjectId::null(), Ordering::Relaxed) == self.list_id);
        let mut cursor = unsafe { self.inner.cursor_mut_from_ptr(Arc::as_ptr(entry)) };
        let entry = cursor.remove().unwrap();
        self.len -= 1;
        entry
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn iter(&self) -> Iter<LinkedListAdapter<T>> {
        self.inner.iter()
    }
}

impl<T: Copy> LruList<T> {
    pub fn to_vec(&self) -> Vec<T> {
        self.iter().map(|entry| entry.inner).collect()
    }
}

impl<T: Copy + Debug> std::fmt::Debug for LruList<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list()
            .entries(self.inner.iter().map(|e| e.inner))
            .finish()
    }
}

impl<T> LruEntry<T> {
    pub fn new(inner: T) -> Self {
        Self {
            inner,
            list_id: Atomic::new(ObjectId::null()),
            link: LinkedListLink::new(),
        }
    }

    pub fn inner(&self) -> &T {
        &self.inner
    }

    pub fn inner_mut(&mut self) -> &mut T {
        &mut self.inner
    }
}

// TODO: scrutinize this use of unsafe
unsafe impl<T: Sync> Sync for LruEntry<T> {}
unsafe impl<T: Send> Send for LruEntry<T> {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basics() {
        let mut list = LruList::<i32>::new();
        let entries: Vec<Arc<LruEntry<i32>>> = (0..5).map(|i| Arc::new(LruEntry::new(i))).collect();
        for entry in &entries {
            list.insert(entry.clone());
        }
        assert_eq!(list.to_vec(), vec![4, 3, 2, 1, 0]);

        list.touch(&entries[0]);
        assert_eq!(list.to_vec(), vec![0, 4, 3, 2, 1]);

        list.touch(&entries[2]);
        assert_eq!(list.to_vec(), vec![2, 0, 4, 3, 1]);

        list.remove(&entries[0]);
        assert_eq!(list.to_vec(), vec![2, 4, 3, 1]);
        assert_eq!(list.len(), 4);

        let lru_entry = list.evict().unwrap();
        assert_eq!(list.to_vec(), vec![2, 4, 3]);
        assert_eq!(*lru_entry.inner(), 1);

        let evicted_entries: Vec<i32> = list
            .evict_nr_with(list.len(), |num| 2 <= *num && *num <= 3)
            .into_iter()
            .map(|entry| *entry.inner())
            .collect();
        assert_eq!(evicted_entries, vec![3, 2] /* From LRU to MRU */);
        assert_eq!(list.to_vec(), vec![4]);
    }
}
