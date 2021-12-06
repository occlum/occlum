// Copyright (C) 2020 Ant Financial Services Group. All rights reserved.

// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions
// are met:

//  * Redistributions of source code must retain the above copyright
//    notice, this list of conditions and the following disclaimer.
//  * Redistributions in binary form must reproduce the above copyright
//    notice, this list of conditions and the following disclaimer in
//    the documentation and/or other materials provided with the
//    distribution.
//  * Neither the name of Ant Financial Services Group nor the names
//    of its contributors may be used to endorse or promote products
//    derived from this software without specific prior written
//    permission.

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

// The design of the implementation is from musl libc:
// Copyright 2005-2019 Rich Felker, et al.

// Permission is hereby granted, free of charge, to any person obtaining
// a copy of this software and associated documentation files (the
// "Software"), to deal in the Software without restriction, including
// without limitation the rights to use, copy, modify, merge, publish,
// distribute, sublicense, and/or sell copies of the Software, and to
// permit persons to whom the Software is furnished to do so, subject to
// the following conditions:

// The above copyright notice and this permission notice shall be
// included in all copies or substantial portions of the Software.

// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
// EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.
// IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
// CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT,
// TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE
// SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

use super::*;

use crate::process::{futex_wait, futex_wake};
use std::hint;
use std::sync::atomic::{AtomicI32, Ordering};

// The implementaion of RwLock
//
// rw_lock: the highest bit holds the "last minute" waiter flag.
//          The other bits hold the write lock state and the reader count:
//          0              means no one is holding the lock,
//          0x7FFF_FFFF    means a writer is holding the lock,
//          1..0x7FFF_FFFE means the number of readers holding the lock.
//
// rw_waiters: the number of the lock waiters which can be readers or writers
//
#[derive(Debug)]
pub struct RwLockInner {
    rw_lock: AtomicI32,
    rw_waiters: AtomicI32,
}

impl RwLockInner {
    pub fn new() -> Self {
        Self {
            rw_lock: AtomicI32::new(0),
            rw_waiters: AtomicI32::new(0),
        }
    }

    pub fn read(&self) -> Result<()> {
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

        // Spin shortly for a probably approaching lock release
        // if no one is waiting but the lock is held
        let mut spins: i32 = 100;
        while spins != 0
            && self.rw_lock.load(Ordering::SeqCst) != 0
            && self.rw_waiters.load(Ordering::SeqCst) == 0
        {
            hint::spin_loop();
            spins -= 1;
        }

        loop {
            let mut ret = self.try_read();
            if let Err(error) = &ret {
                if error.errno() == Errno::EAGAIN {
                    return ret;
                }
            } else {
                return ret;
            }

            let val: i32 = self.rw_lock.load(Ordering::SeqCst);
            if (val & 0x7FFF_FFFF) != 0x7FFF_FFFF {
                continue;
            }

            // Add rw_waiters before setting rw_lock to not to miss any waiters
            // after the waiter flag is set
            self.rw_waiters.fetch_add(1, Ordering::SeqCst);

            let tmp = (val as u32 | 0x8000_0000) as i32;
            self.rw_lock
                .compare_exchange(val, tmp, Ordering::SeqCst, Ordering::SeqCst);
            ret = futex_wait(&self.rw_lock as *const _ as *const i32, tmp, &None);

            self.rw_waiters.fetch_sub(1, Ordering::SeqCst);

            if let Err(error) = &ret {
                match error.errno() {
                    Errno::ECANCELED => return ret,
                    _ => (),
                }
            }
        }
    }

    pub fn try_read(&self) -> Result<()> {
        loop {
            let val: i32 = self.rw_lock.load(Ordering::SeqCst);
            let cnt: i32 = val & 0x7FFF_FFFF;
            if cnt == 0x7FFF_FFFF {
                return_errno!(EBUSY, "a writer is holding the lock");
            }
            if cnt == 0x7FFF_FFFE {
                return_errno!(EAGAIN, "the maximum number of read locks has been exceeded");
            }

            if self
                .rw_lock
                .compare_exchange(val, val + 1, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                break;
            }
        }
        Ok(())
    }

    pub fn write(&self) -> Result<()> {
        let ret = self.try_write();
        if ret.is_ok() {
            return Ok(());
        }

        let mut spins: i32 = 100;
        while spins != 0
            && self.rw_lock.load(Ordering::SeqCst) != 0
            && self.rw_waiters.load(Ordering::SeqCst) == 0
        {
            hint::spin_loop();
            spins -= 1;
        }

        loop {
            let mut ret = self.try_write();
            if ret.is_ok() {
                return Ok(());
            }

            let val = self.rw_lock.load(Ordering::SeqCst);
            if val == 0 {
                continue;
            }

            self.rw_waiters.fetch_add(1, Ordering::SeqCst);

            let tmp = (val as u32 | 0x8000_0000) as i32;
            self.rw_lock
                .compare_exchange(val, tmp, Ordering::SeqCst, Ordering::SeqCst);
            ret = futex_wait(&self.rw_lock as *const _ as *const i32, tmp, &None);

            self.rw_waiters.fetch_sub(1, Ordering::SeqCst);

            if let Err(error) = &ret {
                match error.errno() {
                    Errno::ECANCELED => return ret,
                    _ => (),
                }
            }
        }
    }

    pub fn try_write(&self) -> Result<()> {
        if self
            .rw_lock
            .compare_exchange(0, 0x7FFF_FFFF, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            Ok(())
        } else {
            Err(errno!(EBUSY, "the lock is held for reading or writing"))
        }
    }

    pub fn rw_unlock(&self) -> Result<()> {
        let mut val: i32 = 0;
        let mut cnt: i32 = 0;
        let mut waiters: i32 = 0;
        let mut new: i32 = 0;
        loop {
            // Reverse access order to rw_lock and rw_waiters of that in lock
            val = self.rw_lock.load(Ordering::SeqCst);
            cnt = val & 0x7FFF_FFFF;
            waiters = self.rw_waiters.load(Ordering::SeqCst);
            new = match cnt {
                1 | 0x7FFF_FFFF => 0,
                // val - 1 applies to both positive and negative value as:
                // (i32 & 0x7FFF_FFFF) -1 = (i32 - 1) & 0x7FFF_FFFF
                _ => val - 1,
            };

            if self
                .rw_lock
                .compare_exchange(val, new, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                break;
            }
        }

        // Use both waiters and val in the condition to trigger the wake as much as possible
        // and also to guard the situation where the number of waiters overflows to zero
        if new == 0 && (waiters != 0 || val < 0) {
            // The reasons to use cnt other than waiters here:
            // For read_unlock, only one waiter which must be a writer needs to be waken;
            // For write_unlock, at most 0x7FFF_FFFF waiters can be waken.
            futex_wake(&self.rw_lock as *const _ as *const i32, cnt as usize);
        }
        Ok(())
    }

    pub fn read_unlock(&self) -> Result<()> {
        self.rw_unlock()
    }

    pub fn write_unlock(&self) -> Result<()> {
        self.rw_unlock()
    }

    pub fn destroy(&self) -> Result<()> {
        Ok(())
    }
}
