use core::fmt;

use super::{Errno, Error};

pub trait ToErrno: fmt::Display + fmt::Debug {
    fn errno(&self) -> Errno;
}

impl<T> From<T> for Error
where
    T: ToErrno + 'static,
{
    fn from(t: T) -> Error {
        Error::boxed(t, None)
    }
}

impl ToErrno for Errno {
    fn errno(&self) -> Errno {
        *self
    }
}

impl ToErrno for core::alloc::AllocErr {
    fn errno(&self) -> Errno {
        Errno::ENOMEM
    }
}

impl ToErrno for core::alloc::LayoutErr {
    fn errno(&self) -> Errno {
        Errno::EINVAL
    }
}

impl ToErrno for core::num::ParseIntError {
    fn errno(&self) -> Errno {
        Errno::EINVAL
    }
}

#[cfg(any(feature = "std", feature = "sgx", test, doctest))]
mod if_std {
    use super::*;

    impl From<std::io::ErrorKind> for Errno {
        fn from(kind: std::io::ErrorKind) -> Errno {
            use std::io::ErrorKind::*;
            use Errno::*;
            match kind {
                NotFound => ENOENT,
                PermissionDenied => EPERM,
                ConnectionRefused => ECONNREFUSED,
                ConnectionReset => ECONNRESET,
                ConnectionAborted => ECONNABORTED,
                NotConnected => ENOTCONN,
                AddrInUse => EADDRINUSE,
                AddrNotAvailable => EADDRNOTAVAIL,
                BrokenPipe => EPIPE,
                AlreadyExists => EEXIST,
                WouldBlock => EWOULDBLOCK,
                InvalidInput => EINVAL,
                InvalidData => EBADMSG, /* TODO: correct? */
                TimedOut => ETIMEDOUT,
                Interrupted => EINTR,
                WriteZero => EINVAL,
                UnexpectedEof => EIO,
                Other => EIO,
                _ => EIO,
            }
        }
    }

    impl ToErrno for std::io::Error {
        fn errno(&self) -> Errno {
            Errno::from(self.kind())
        }
    }

    impl ToErrno for std::ffi::NulError {
        fn errno(&self) -> Errno {
            Errno::EINVAL
        }
    }
}

#[cfg(feature = "occlum")]
mod if_occlum {
    use rcore_fs::dev::DevError;
    use rcore_fs::vfs::FsError;

    use super::*;

    impl ToErrno for serde_json::Error {
        fn errno(&self) -> Errno {
            Errno::EINVAL
        }
    }

    impl ToErrno for FsError {
        fn errno(&self) -> Errno {
            use Errno::*;
            match *self {
                FsError::NotSupported => ENOSYS,
                FsError::NotFile => EISDIR,
                FsError::IsDir => EISDIR,
                FsError::NotDir => ENOTDIR,
                FsError::EntryNotFound => ENOENT,
                FsError::EntryExist => EEXIST,
                FsError::NotSameFs => EXDEV,
                FsError::InvalidParam => EINVAL,
                FsError::NoDeviceSpace => ENOMEM,
                FsError::DirRemoved => ENOENT,
                FsError::DirNotEmpty => ENOTEMPTY,
                FsError::WrongFs => EINVAL,
                FsError::DeviceError(_err) => EIO,
                FsError::SymLoop => ELOOP,
                FsError::NoDevice => ENXIO,
                FsError::IOCTLError => EINVAL,
                FsError::Again => EAGAIN,
                FsError::Busy => EBUSY,
                FsError::WrProtected => EROFS,
                FsError::NoIntegrity => EIO,
                FsError::PermError => EPERM,
                FsError::NameTooLong => ENAMETOOLONG,
            }
        }
    }

    impl From<Error> for DevError {
        fn from(e: Error) -> Self {
            DevError(e.errno() as i32)
        }
    }
}
