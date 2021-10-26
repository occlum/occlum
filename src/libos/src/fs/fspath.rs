use super::*;

use std::convert::TryFrom;

pub const AT_FDCWD: i32 = -100;

/// The representation of path in FS
#[derive(Debug)]
pub struct FsPath<'a> {
    inner: FsPathInner<'a>,
}

impl<'a> FsPath<'a> {
    /// Construct a FsPath
    pub fn new(path: &'a str, dirfd: i32) -> Result<Self> {
        Ok(FsPath {
            inner: FsPathInner::new(path, dirfd)?,
        })
    }

    pub(in crate::fs) fn inner(&self) -> &FsPathInner {
        &self.inner
    }
}

impl<'a> TryFrom<&'a str> for FsPath<'a> {
    type Error = errno::Error;

    fn try_from(path: &str) -> Result<FsPath> {
        if path.is_empty() {
            return_errno!(ENOENT, "path is an empty string");
        }
        FsPath::new(path, AT_FDCWD)
    }
}

/// The internal representation of path in FS
#[derive(Debug)]
pub(in crate::fs) enum FsPathInner<'a> {
    /// absolute path
    Absolute(&'a str),
    /// path is relative to cwd
    CwdRelative(&'a str),
    /// cwd
    Cwd,
    /// path is relative to dir fd
    FdRelative(FileDesc, &'a str),
    /// fd itself
    Fd(FileDesc),
}

impl<'a> FsPathInner<'a> {
    pub fn new(path: &'a str, dirfd: i32) -> Result<Self> {
        let fs_path_inner = if Path::new(path).is_absolute() {
            Self::Absolute(path)
        } else if dirfd >= 0 {
            if path.is_empty() {
                Self::Fd(dirfd as FileDesc)
            } else {
                Self::FdRelative(dirfd as FileDesc, path)
            }
        } else if dirfd == AT_FDCWD {
            if path.is_empty() {
                Self::Cwd
            } else {
                Self::CwdRelative(path)
            }
        } else {
            return_errno!(EINVAL, "invalid dirfd number");
        };

        Ok(fs_path_inner)
    }
}
