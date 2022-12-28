use std::collections::HashSet;

use crate::prelude::*;

/// ThreadId implements self-managed thread IDs.
///
/// Each instance of ThreadID are guaranteed to have a unique ID.
/// And when a ThreadID instance is freed, its ID is automatically freed too.
#[derive(Debug, PartialEq)]
pub struct ThreadId {
    pub tid: u32,
}

impl ThreadId {
    /// Create a new thread ID.
    ///
    /// The thread ID returned is guaranteed to have a value greater than zero.
    pub fn new() -> ThreadId {
        let mut alloc = THREAD_ID_ALLOC.lock().unwrap();
        let tid = alloc.alloc();
        Self { tid }
    }

    /// Create a "zero" thread ID.
    ///
    /// This "zero" thread ID is used exclusively by the idle process.
    pub fn zero() -> ThreadId {
        Self { tid: 0 }
    }

    /// Return the value of the thread ID.
    pub fn as_u32(&self) -> u32 {
        self.tid
    }
}

impl Drop for ThreadId {
    fn drop(&mut self) {
        if self.tid == 0 {
            return;
        }

        let mut alloc = THREAD_ID_ALLOC.lock().unwrap();
        alloc.free(self.tid);
    }
}

lazy_static! {
    static ref THREAD_ID_ALLOC: SgxMutex<IdAlloc> = SgxMutex::new(IdAlloc::new());
}

/// PID/TID allocator.
///
/// The allocation strategy is to start from the minimal value (here, 1) and increments
/// each returned ID, until a maximum value (e.g., 2^32-1) is reached. After that, recycle
/// from the minimal value and see if it is still in use. If not, use the value; otherwise,
/// increments again.
///
/// The allocation strategy above follows the *nix tradition.
///
/// Note that PID/TID 0 is reserved for the idle process. So the id allocator starts from 1.
#[derive(Debug, Clone)]
struct IdAlloc {
    next_id: u32,
    used_ids: HashSet<u32>,
}

impl IdAlloc {
    pub fn new() -> Self {
        Self {
            next_id: 0,
            used_ids: HashSet::new(),
        }
    }

    pub fn alloc(&mut self) -> u32 {
        let new_id = loop {
            // Increments the ID and wrap around if necessary
            self.next_id = self.next_id.wrapping_add(1);
            if self.next_id == 0 {
                self.next_id = 1;
            }

            if !self.used_ids.contains(&self.next_id) {
                break self.next_id;
            }
        };
        self.used_ids.insert(new_id);
        new_id
    }

    pub fn free(&mut self, id: u32) -> Option<u32> {
        // Note: When enabling "execve", there is situation that the ThreadId is reused.
        // And thus when exit, it may free twice.
        // debug_assert!(self.used_ids.contains(&id));
        if self.used_ids.remove(&id) {
            Some(id)
        } else {
            None
        }
    }
}
