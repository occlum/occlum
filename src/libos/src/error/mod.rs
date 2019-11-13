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
        let error = Error::embeded(inner_error, Some(ErrorLocation::new(file!(), line!())));
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
