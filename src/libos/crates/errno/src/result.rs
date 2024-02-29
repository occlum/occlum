use super::{Errno, Error};

pub type Result<T> = core::result::Result<T, Error>;

/// Extending `Result` with extra functionalities.
pub trait ResultExt<T> {
    fn cause_err<F>(self, f: F) -> Result<T>
    where
        F: FnOnce(&Error) -> Error;

    fn errno(&self) -> Option<Errno>;

    fn has_errno(&self, errno: Errno) -> bool;
}

impl<T> ResultExt<T> for Result<T> {
    fn cause_err<F>(self, f: F) -> Result<T>
    where
        F: FnOnce(&Error) -> Error,
    {
        self.map_err(|old_e| old_e.cause_err(f))
    }

    fn errno(&self) -> Option<Errno> {
        match self {
            Ok(_) => None,
            Err(e) => Some(e.errno()),
        }
    }

    fn has_errno(&self, errno: Errno) -> bool {
        self.errno() == Some(errno)
    }
}
