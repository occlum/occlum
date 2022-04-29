use core::ptr::NonNull;

use intrusive_collections::intrusive_adapter;
use intrusive_collections::{LinkedList, LinkedListLink};
use object_id::ObjectId;

use super::waiter::{Waiter, WaiterInner};
use crate::prelude::*;

/// A waiter queue.
///
/// The queue keeps a list of waiters so that the user can wake them up at a
/// proper timing.
///
/// The queue is fair in the sense that it guarantees that when waking up
/// waiters, we have
/// * Older waiters get higher priority than newer waiters;
/// * Newer waiters get their chances to be woken up eventually.
///
/// To do so, the queue maintains an internal cursor to the next waiter to be
/// woken up.
#[derive(Debug)]
pub struct WaiterQueue {
    inner: Mutex<WaiterQueueInner>,
}

impl WaiterQueue {
    /// Create a new instance.
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(WaiterQueueInner::new()),
        }
    }

    pub(crate) fn inner(&self) -> &Mutex<WaiterQueueInner> {
        &self.inner
    }

    /// Enqueue a waiter.
    pub fn enqueue(&self, waiter: &mut Waiter) {
        let mut inner = self.inner.lock();
        inner.enqueue(waiter)
    }

    /// Dequeue a waiter.
    pub fn dequeue(&self, waiter: &mut Waiter) {
        let mut inner = self.inner.lock();
        inner.dequeue(waiter);
    }

    /// Wake up all waiters in the queue.
    pub fn wake_all(&self) -> usize {
        self.wake_nr(usize::max_value())
    }

    /// Wake up at most one waiter.
    pub fn wake_one(&self) -> usize {
        self.wake_nr(1)
    }

    /// Wake up a given number of waiters.
    pub fn wake_nr(&self, count: usize) -> usize {
        let mut inner = self.inner.lock();
        inner.wake_nr(count)
    }
}

#[derive(Debug)]
pub(crate) struct WaiterQueueInner {
    list: LinkedList<LinkedListAdapter>,
    next_ptr: Option<NonNull<WaiterInner>>,
    id: ObjectId,
}

unsafe impl Send for WaiterQueueInner {}
unsafe impl Sync for WaiterQueueInner {}

intrusive_adapter!(LinkedListAdapter =
    Arc<WaiterInner>: WaiterInner {
        link: LinkedListLink
    }
);

impl WaiterQueueInner {
    pub fn new() -> Self {
        Self {
            list: LinkedList::new(LinkedListAdapter::new()),
            next_ptr: None,
            id: ObjectId::new(),
        }
    }

    pub fn enqueue(&mut self, waiter: &Waiter) {
        // Ensure that the waiter has not been queued to any other waiter queue.
        let curr_queue_id = waiter.inner().queue_id().swap(self.id, Ordering::Relaxed);
        assert!(curr_queue_id == ObjectId::null());

        self.list.push_back(waiter.inner().clone());
    }

    pub fn dequeue(&mut self, waiter: &Waiter) {
        // Ensure that the waiter won't be dequeued twice.
        let curr_queue_id = waiter
            .inner()
            .queue_id()
            .swap(ObjectId::null(), Ordering::Relaxed);
        assert!(curr_queue_id == self.id);

        let ptr = Arc::as_ptr(waiter.inner());
        let mut cursor = unsafe { self.list.cursor_mut_from_ptr(ptr) };
        let waiter_inner = cursor.remove().unwrap();
        drop(waiter_inner);

        // If the dequeued item happens to be the same item refered to by
        // self.next_ptr, then we move self.next_ptr to the next item.
        if let Some(next_ptr) = self.next_ptr.map(|p| p.as_ptr() as *const _) {
            if next_ptr == ptr {
                self.next_ptr = cursor.get().map(|waiter_inner| {
                    let next_raw_ptr = waiter_inner as *const _ as *mut _;
                    unsafe { NonNull::new_unchecked(next_raw_ptr) }
                });
            }
        }
    }

    pub fn wake_nr(&mut self, max_count: usize) -> usize {
        if max_count == 0 {
            return 0;
        }

        // Make sure self.ptr refers to a valid items in the list
        if self.next_ptr.is_none() {
            let cursor = self.list.front_mut();
            if cursor.is_null() {
                return 0;
            }

            let raw_ptr = cursor
                .get()
                .map(|waiter_inner| waiter_inner as *const _ as *mut _)
                .unwrap();
            self.next_ptr = Some(unsafe { NonNull::new_unchecked(raw_ptr) });
        }

        let end_ptr = self.next_ptr.map(|ptr| ptr.as_ptr()).unwrap();
        let mut cursor = unsafe { self.list.cursor_from_ptr(end_ptr) };
        let mut count = 0;
        loop {
            // Wake up one waiter
            let waiter_inner = cursor.get().unwrap();
            if waiter_inner.wake().is_some() {
                count += 1;
            }

            // Move on to the next one
            cursor.move_next();
            if cursor.is_null() {
                // Since the list has at least one item, we can be sure that after
                // moving next, the cursor won't be null.
                cursor.move_next();
            }

            // Check termination condition
            let ptr = cursor
                .get()
                .map(|waiter_inner| waiter_inner as *const _)
                .unwrap();
            if ptr == end_ptr {
                // Don't need to update self.next_ptr
                return count;
            }
            if count == max_count {
                self.next_ptr = Some(unsafe { NonNull::new_unchecked(ptr as *mut _) });
                return count;
            }
        }
    }
}
