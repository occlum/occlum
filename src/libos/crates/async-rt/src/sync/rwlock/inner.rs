// Copyright (C) 2019 - 2023 The Occlum Project. All rights reserved.
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions
// are met:
//
//  * Redistributions of source code must retain the above copyright
//    notice, this list of conditions and the following disclaimer.
//  * Redistributions in binary form must reproduce the above copyright
//    notice, this list of conditions and the following disclaimer in
//    the documentation and/or other materials provided with the
//    distribution.
//  * Neither the name of copyright holder nor the names
//    of its contributors may be used to endorse or promote products
//    derived from this software without specific prior written
//    permission.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS
// "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
// LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
// A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT
// OWNER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
// SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT
// LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE,
// DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY
// THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
// (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
// OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
//
// The design of the implementation is from musl libc:
// Copyright 2005-2019 Rich Felker, et al.
//
// Permission is hereby granted, free of charge, to any person obtaining
// a copy of this software and associated documentation files (the
// "Software"), to deal in the Software without restriction, including
// without limitation the rights to use, copy, modify, merge, publish,
// distribute, sublicense, and/or sell copies of the Software, and to
// permit persons to whom the Software is furnished to do so, subject to
// the following conditions:
//
// The above copyright notice and this permission notice shall be
// included in all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
// EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.
// IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
// CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT,
// TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE
// SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

use super::hint;
use super::*;

use std::sync::atomic::AtomicI32;

use crate::wait::{Waiter, WaiterQueue};

/// The number of spinning time before sleeping
/// In musl's implmenetation, this is `100`. Considering more overhead in SGX environment,
/// here we make it bigger.
const SPIN_COUNT: usize = 1000;

/// The implementaion of RwLock
///
/// rw_lock: the highest bit holds the "last minute" waiter flag.
///          The other bits hold the write lock state and the reader count:
///          0              means no one is holding the lock,
///          0x7FFF_FFFF    means a writer is holding the lock,
///          1..0x7FFF_FFFE means the number of readers holding the lock.
///
/// rw_waiters: the number of the lock waiters which can be readers or writers
///
#[derive(Debug)]
pub struct RwLockInner {
    status: AtomicRwLockStatus,
    rw_waiters: AtomicI32,
    waiter_queue: WaiterQueue,
}

#[derive(Debug)]
// This struct is the atomic wrapper for RwLockStatus.
struct AtomicRwLockStatus(AtomicI32);

#[derive(Debug, Copy, Clone)]
#[repr(i32)]
enum RwLockStatus {
    Free = 0,                // Set by unlocking thread.
    ReaderLocked(i32), // Set by locking thread. The number indicates the number of readers who hold the lock
    Waiting(i32), // Set by waiting thread. The number is nagative and indicates whether it is reader locked or writer locked.
    WriterLocked = i32::MAX, // Set by locking thread. A writer is holding the lock.
}

impl RwLockInner {
    pub fn new() -> Self {
        Self {
            status: AtomicRwLockStatus::new(),
            rw_waiters: AtomicI32::new(0),
            waiter_queue: WaiterQueue::new(),
        }
    }

    pub async fn read(&self) -> Result<()> {
        let ret = self.try_read();
        if let Err(error) = &ret {
            // Return error if the reader number reaches the limit of i32
            if error.errno() == Errno::EAGAIN {
                return ret;
            }
        } else {
            // Return ok if try_lock succeeds
            return ret;
        }

        // Spin shortly for a probably approaching lock release if no one is waiting but the lock is held
        let mut spins = SPIN_COUNT;
        while spins != 0
            && self.status.is_locked()
            // Can't reorder here. `Relaxed` is enough.
            && self.rw_waiters.load(Ordering::Relaxed) == 0
        {
            hint::spin_loop();
            spins -= 1;
        }

        loop {
            let ret = self.try_read();
            if let Err(error) = &ret {
                if error.errno() == Errno::EAGAIN {
                    return ret;
                }
            } else {
                return ret;
            }

            // Check status again
            let current_status = self.status();
            match current_status.get_locker() {
                // If it is free or locked by readers, try_read should success.
                RwLockStatus::Free | RwLockStatus::ReaderLocked(_) => {
                    continue;
                }
                _ => {}
            }

            // Someone is holding the write lock. Need to wait.
            debug_assert!(current_status.get_locker() == RwLockStatus::WriterLocked);

            // Add rw_waiters before setting status to not to miss any waiters after the waiting flag is set
            // This can be reordered and in try_set_new_status, `AcqRel` will make sure this happens before.
            self.rw_waiters.fetch_add(1, Ordering::Relaxed);

            // new_status indicates whether it is wait for reader lock or writer lock
            let new_status = current_status.set_waiting();

            // Ignore the result here because if setting the new_status fails, the wait will not block.
            self.status
                .try_set_new_status(current_status, new_status)
                .map_err(|_| warn!("failed to set RwLock status"))
                .ok();
            self.wait(new_status).await;

            self.rw_waiters.fetch_sub(1, Ordering::Relaxed);
        }
    }

    pub fn try_read(&self) -> Result<()> {
        loop {
            let current_status = self.status();
            let locker = current_status.get_locker();
            match locker {
                RwLockStatus::Free => {}
                RwLockStatus::WriterLocked => {
                    return_errno!(EBUSY, "a writer is holding the lock");
                }
                RwLockStatus::ReaderLocked(cnt) => {
                    if cnt == RwLockStatus::max_read_lock_holder_num() {
                        return_errno!(EAGAIN, "the maximum number of read locks has reached");
                    }
                }
                _ => unreachable!(),
            }

            if self.status.try_add_one_reader(current_status).is_ok() {
                break;
            }
        }
        Ok(())
    }

    pub async fn write(&self) -> Result<()> {
        if let Ok(_) = self.try_write() {
            return Ok(());
        }

        let mut spins = SPIN_COUNT;
        while spins != 0
            && self.status.is_locked()
            // Can't reorder here. `Relaxed` is enough.
            && self.rw_waiters.load(Ordering::Relaxed) == 0
        {
            hint::spin_loop();
            spins -= 1;
        }

        loop {
            if let Ok(_) = self.try_write() {
                return Ok(());
            }

            let status = self.status();
            if status == RwLockStatus::Free {
                continue;
            }

            // Add rw_waiters before setting status to not to miss any waiters after the waiting flag is set.
            // This can be reordered and in try_set_new_status, `AcqRel` will make sure this happens before.
            self.rw_waiters.fetch_add(1, Ordering::Relaxed);

            let new_status = status.set_waiting();
            self.status
                .try_set_new_status(status, new_status)
                .map_err(|_| warn!("failed to set RwLock status"))
                .ok();
            self.wait(new_status).await;

            self.rw_waiters.fetch_sub(1, Ordering::Relaxed);
        }
    }

    pub fn try_write(&self) -> Result<()> {
        if self.status.try_set_writer_locked().is_ok() {
            Ok(())
        } else {
            Err(errno!(EBUSY, "the lock is held for reading or writing"))
        }
    }

    pub fn rw_unlock(&self) -> Result<()> {
        let mut status;
        let mut new_status;
        let waiters;
        // Set status to Free or subtract one reader lock holder
        loop {
            status = self.status();
            new_status = match status.get_lock_holder_num() {
                1 => RwLockStatus::Free,
                // status - 1 applies to both positive and negative value as:
                // (i32 & 0x7FFF_FFFF) -1 = (i32 - 1) & 0x7FFF_FFFF
                _ => (status.as_i32() - 1).try_into().unwrap(),
            };

            if self.status.try_set_new_status(status, new_status).is_ok() {
                // This can't be reordered. `Relaxed` is enough.
                waiters = self.rw_waiters.load(Ordering::Relaxed);
                break;
            }
        }

        // Use both waiters and val in the condition to trigger the wake as much as possible
        // and also to guard the situation where the number of waiters overflows to zero
        if new_status == RwLockStatus::Free && (waiters != 0 || status.is_waiting()) {
            let wake_num = status.get_waking_num();
            self.wake(wake_num);
        }
        Ok(())
    }

    pub fn read_unlock(&self) -> Result<()> {
        self.rw_unlock()
    }

    pub fn write_unlock(&self) -> Result<()> {
        self.rw_unlock()
    }

    fn status(&self) -> RwLockStatus {
        // Use `Acquire` here to make sure all memory access before are completed.
        self.status.0.load(Ordering::Acquire).try_into().unwrap()
    }

    async fn wait(&self, new_status: RwLockStatus) {
        let mut waiter = Waiter::new();

        // Acquire the lock for waiter queue and check the status again to make sure the status is expected before waiting.
        let mut locked_waiter_queue = self.waiter_queue.inner().lock();
        // Check the status value again
        let status = self.status();
        if status != new_status {
            return;
        }

        locked_waiter_queue.enqueue(&mut waiter);

        drop(locked_waiter_queue);
        let _ = waiter.wait().await;

        self.waiter_queue.dequeue(&mut waiter);
    }

    fn wake(&self, wake_num: i32) {
        debug_assert!(wake_num > 0);
        self.waiter_queue.wake_nr(wake_num as usize);
    }
}

// For AtomicRwLockStatus, global ordering is not needed. `Acquire` and `Release` are enough for the atomic operations.
impl AtomicRwLockStatus {
    fn new() -> Self {
        Self(AtomicI32::new(RwLockStatus::new().as_i32()))
    }

    fn is_free(&self) -> bool {
        self.0.load(Ordering::Acquire) == RwLockStatus::Free.as_i32()
    }

    fn is_locked(&self) -> bool {
        self.0.load(Ordering::Acquire) != RwLockStatus::Free.as_i32()
    }

    fn is_reader_locked(&self) -> bool {
        self.0.load(Ordering::Acquire) & 0x7FFF_FFFF != 0x7FFF_FFFF
    }

    fn try_add_one_reader(&self, current_status: RwLockStatus) -> Result<()> {
        let status_raw = current_status.as_i32();
        if let Err(_) = self.0.compare_exchange(
            status_raw,
            status_raw + 1,
            Ordering::AcqRel,
            Ordering::Relaxed,
        ) {
            return_errno!(EAGAIN, "current status changed, try again");
        } else {
            Ok(())
        }
    }

    fn try_set_writer_locked(&self) -> Result<()> {
        if let Err(_) = self.0.compare_exchange(
            RwLockStatus::Free.as_i32(),
            RwLockStatus::WriterLocked.as_i32(),
            Ordering::AcqRel,
            Ordering::Relaxed,
        ) {
            return_errno!(EBUSY, "try set writer locked failed");
        } else {
            Ok(())
        }
    }

    fn try_set_new_status(
        &self,
        current_status: RwLockStatus,
        new_status: RwLockStatus,
    ) -> Result<()> {
        if let Err(_) = self.0.compare_exchange(
            current_status.as_i32(),
            new_status.as_i32(),
            Ordering::AcqRel,
            Ordering::Relaxed, // We don't care failure thus make it `Relaxed`.
        ) {
            return_errno!(EAGAIN, "try set waiting failed");
        }
        Ok(())
    }
}

impl RwLockStatus {
    fn new() -> Self {
        RwLockStatus::Free
    }

    fn max_read_lock_holder_num() -> i32 {
        i32::MAX - 1 // 0x7FFF_FFFE
    }

    fn get_locker(&self) -> RwLockStatus {
        let num = self.as_i32() & 0x7FFF_FFFF;
        debug_assert!(num >= 0);
        match num {
            0 => RwLockStatus::Free,
            i32::MAX => RwLockStatus::WriterLocked,
            _ => RwLockStatus::ReaderLocked(num),
        }
    }

    fn get_lock_holder_num(&self) -> i32 {
        let locker = self.get_locker();
        match locker {
            RwLockStatus::Free => return 0,
            // One reader holder or one writer holder
            RwLockStatus::ReaderLocked(1) | RwLockStatus::WriterLocked => return 1,
            RwLockStatus::ReaderLocked(num) => return num,
            _ => unreachable!(), // can't be Waiting
        }
    }

    fn get_waking_num(&self) -> i32 {
        let locker = self.get_locker();
        match locker {
            // For write_unlock, wake as much as possible (i32::MAX)
            RwLockStatus::WriterLocked => RwLockStatus::WriterLocked.as_i32(),
            // For reader_unlock (last read lock holder), only one waiter which must be a writer needs to be waken;
            RwLockStatus::ReaderLocked(num) => {
                debug_assert!(num == 1);
                return num;
            }
            // This function are supposed to be called only in wake(). For other situations, wake should not be called.
            _ => unreachable!(),
        }
    }

    fn is_waiting(&self) -> bool {
        match self {
            RwLockStatus::Waiting(_) => true,
            _ => false,
        }
    }

    #[allow(overflowing_literals)]
    fn set_waiting(&self) -> RwLockStatus {
        RwLockStatus::Waiting(self.as_i32() | 0x8000_0000)
    }

    fn as_i32(&self) -> i32 {
        match self {
            RwLockStatus::Free => 0,
            RwLockStatus::ReaderLocked(num) => *num,
            RwLockStatus::WriterLocked => i32::MAX,
            RwLockStatus::Waiting(num) => *num,
        }
    }
}

impl PartialEq for RwLockStatus {
    fn eq(&self, other: &Self) -> bool {
        self.as_i32() == other.as_i32()
    }
}

impl Eq for RwLockStatus {}

impl TryFrom<i32> for RwLockStatus {
    type Error = Error;

    fn try_from(v: i32) -> Result<Self> {
        match v {
            x if x == RwLockStatus::Free.as_i32() => Ok(RwLockStatus::Free),
            x if x == RwLockStatus::WriterLocked.as_i32() => Ok(RwLockStatus::WriterLocked),
            x if x > RwLockStatus::Free.as_i32() && x < RwLockStatus::WriterLocked.as_i32() => {
                Ok(RwLockStatus::ReaderLocked(x))
            }
            // negative means someone is waiting, and we also need to keep track of the number of lock holders
            x if x < RwLockStatus::Free.as_i32() => Ok(RwLockStatus::Waiting(x)),
            _ => return_errno!(EINVAL, "Invalid RwLock status"),
        }
    }
}
