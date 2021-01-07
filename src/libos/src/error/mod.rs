use super::*;
use std::fmt;

mod backtrace;
mod errno;
mod error;
mod to_errno;

pub use self::backtrace::{ErrorBacktrace, ResultExt};
pub use self::errno::Errno;
pub use self::errno::Errno::*;
pub use self::error::{Error, ErrorLocation};
pub use self::to_errno::ToErrno;

pub type Result<T> = std::result::Result<T, Error>;

macro_rules! errno {
    ($errno_expr: expr, $error_msg: expr) => {{
        let inner_error = {
            let errno: Errno = $errno_expr;
            let msg: &'static str = $error_msg;
            (errno, msg)
        };
        let error = Error::embedded(inner_error, Some(ErrorLocation::new(file!(), line!())));
        error
    }};
    ($error_expr: expr) => {{
        let inner_error = $error_expr;
        let error = Error::boxed(inner_error, Some(ErrorLocation::new(file!(), line!())));
        error
    }};
}

macro_rules! return_errno {
    ($errno_expr: expr, $error_msg: expr) => {{
        return Err(errno!($errno_expr, $error_msg));
    }};
    ($error_expr: expr) => {{
        return Err(errno!($error_expr));
    }};
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
