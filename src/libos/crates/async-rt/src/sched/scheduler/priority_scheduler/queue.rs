use crate::prelude::*;
use crate::task::Task;
use flume::{Receiver, Sender, TrySendError};

pub(crate) struct PeekableTaskQueue {
    slot: Mutex<Option<Arc<Task>>>,
    queue: TaskQueue,
}

impl PeekableTaskQueue {
    pub fn new(capacity: Option<usize>) -> Self {
        Self {
            slot: Mutex::new(None),
            queue: TaskQueue::new(capacity),
        }
    }

    pub fn push(&self, task: Arc<Task>) -> core::result::Result<(), Arc<Task>> {
        self.queue.push(task)
    }

    pub fn pop(&self) -> Option<Arc<Task>> {
        // TODO: maybe use try_lock() instead of lock(), hence we can get a fast path.
        let mut guard = self.slot.lock();
        if guard.is_some() {
            return guard.take();
        }
        drop(guard);

        self.queue.pop()
    }

    /// Pop the front task if the front task passed the check function.
    ///
    /// Return the result of check function and the front task if check function returns `Some`.
    /// Return `None` if check function returns `None`.
    pub fn pop_if_pass_check<T, F>(&self, f: F) -> Option<(T, Arc<Task>)>
    where
        F: FnOnce(&Arc<Task>) -> Option<T>,
    {
        let mut guard = self.slot.lock();
        if guard.is_none() {
            if let Some(task) = self.queue.pop() {
                *guard = Some(task);
            } else {
                return None;
            }
        }

        f(guard.as_ref().unwrap()).map_or(None, |idx| Some((idx, guard.take().unwrap())))
    }

    pub fn capacity(&self) -> Option<usize> {
        self.queue.capacity()
    }

    pub fn is_empty(&self) -> bool {
        if self.queue.is_empty() {
            let guard = self.slot.lock();
            return guard.is_none();
        }
        false
    }

    /// Returns the length of queue.
    ///
    /// The length might not be accurate, if the accurate len is real_len,
    /// the length we returned is real_len or real_len - 1
    pub fn len(&self) -> usize {
        self.queue.len()
    }
}

pub(crate) struct TaskQueue {
    recv: Receiver<Arc<Task>>,
    send: Sender<Arc<Task>>,
    capacity: Option<usize>,
}

impl TaskQueue {
    pub fn new(bound: Option<usize>) -> Self {
        let (send, recv) = match bound {
            Some(size) => flume::bounded(size),
            None => flume::unbounded(),
        };

        Self {
            recv,
            send,
            capacity: bound,
        }
    }

    pub fn push(&self, task: Arc<Task>) -> core::result::Result<(), Arc<Task>> {
        if self.capacity.is_some() {
            match self.send.try_send(task) {
                Ok(_) => Ok(()),
                Err(e) => match e {
                    TrySendError::Full(t) => Err(t),
                    TrySendError::Disconnected(_) => {
                        panic!("the channel of flume is disconnected.")
                    }
                },
            }
        } else {
            self.send.send(task).unwrap();
            Ok(())
        }
    }

    pub fn pop(&self) -> Option<Arc<Task>> {
        if let Ok(task) = self.recv.try_recv() {
            return Some(task);
        }
        return None;
    }

    pub fn capacity(&self) -> Option<usize> {
        self.capacity
    }

    pub fn len(&self) -> usize {
        self.recv.len()
    }

    pub fn is_empty(&self) -> bool {
        self.recv.is_empty()
    }
}
