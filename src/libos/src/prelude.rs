pub use sgx_trts::libc;
pub use sgx_trts::libc::off_t;
pub use sgx_types::*;

use std;
pub use std::cell::{Cell, RefCell};
pub use std::cmp::{max, min};
pub use std::collections::{HashMap, VecDeque};
pub use std::fmt::{Debug, Display};
pub use std::ops::{Deref, DerefMut};
pub use std::prelude::v1::*;
pub use std::sync::{
    Arc, SgxMutex, SgxMutexGuard, SgxRwLock, SgxRwLockReadGuard, SgxRwLockWriteGuard,
};

pub use crate::fs::{File, FileDesc, FileRef};
pub use crate::process::{pid_t, uid_t};
pub use crate::util::sync::RwLock;
pub use errno::prelude::*;

// To override the default Result type
pub use errno::prelude::Result;

macro_rules! debug_trace {
    () => {
        debug!("> Line = {}, File = {}", line!(), file!())
    };
}

macro_rules! current {
    () => {
        crate::process::current::get()
    };
}

pub fn align_up(addr: usize, align: usize) -> usize {
    debug_assert!(align != 0 && align.is_power_of_two());
    align_down(addr + (align - 1), align)
}

pub fn align_down(addr: usize, align: usize) -> usize {
    debug_assert!(align != 0 && align.is_power_of_two());
    addr & !(align - 1)
}

pub fn unbox<T>(value: Box<T>) -> T {
    *value
}

// return Err(errno) if libc return -1
macro_rules! try_libc {
    ($ret: expr) => {{
        let ret = unsafe { $ret };
        if ret < 0 {
            let errno = unsafe { libc::errno() };
            return_errno!(Errno::from(errno as u32), "libc error");
        }
        ret
    }};
}

// return Err(errno) if libc return -1
// raise SIGPIPE if errno == EPIPE
macro_rules! try_libc_may_epipe {
    ($ret: expr) => {{
        let ret = unsafe { $ret };
        if ret < 0 {
            let errno = unsafe { libc::errno() };
            if errno == Errno::EPIPE as i32 {
                crate::signal::do_tkill(current!().tid(), crate::signal::SIGPIPE.as_u8() as i32);
            }
            return_errno!(Errno::from(errno as u32), "libc error");
        }
        ret
    }};
}
