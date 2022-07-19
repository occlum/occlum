use super::*;

use std::convert::TryFrom;

pub const AT_FDCWD: i32 = -100;

/// The representation of path in FS
#[derive(Debug)]
pub struct FsPath {
    inner: FsPathInner,
}

impl FsPath {
    /// Construct a FsPath
    pub fn new(path: String, dirfd: i32) -> Result<Self> {
        Ok(FsPath {
            inner: FsPathInner::new(path, dirfd)?,
        })
    }

    pub(in crate::fs) fn inner(&self) -> &FsPathInner {
        &self.inner
    }

    pub fn ends_with(&self, pat: &str) -> bool {
        match &self.inner {
            FsPathInner::Absolute(path) => path.ends_with(pat),
            FsPathInner::CwdRelative(path) => path.ends_with(pat),
            FsPathInner::Cwd => false,
            FsPathInner::FdRelative(_, path) => path.ends_with(pat),
            FsPathInner::Fd(_) => false,
        }
    }

    pub fn trim_end_matches(&self, pat: char) -> Self {
        let trim_inner = match &self.inner {
            FsPathInner::Absolute(path) => {
                FsPathInner::Absolute(path.trim_end_matches(pat).to_string())
            }
            FsPathInner::CwdRelative(path) => {
                FsPathInner::CwdRelative(path.trim_end_matches(pat).to_string())
            }
            FsPathInner::Cwd => FsPathInner::Cwd,
            FsPathInner::FdRelative(fd, path) => {
                FsPathInner::FdRelative(*fd, path.trim_end_matches(pat).to_string())
            }
            FsPathInner::Fd(fd) => FsPathInner::Fd(*fd),
        };
        Self { inner: trim_inner }
    }
}

impl TryFrom<&str> for FsPath {
    type Error = errno::Error;

    fn try_from(path: &str) -> Result<FsPath> {
        if path.is_empty() {
            return_errno!(ENOENT, "path is an empty string");
        }
        FsPath::new(String::from(path), AT_FDCWD)
    }
}

/// The internal representation of path in FS
#[derive(Debug)]
pub(in crate::fs) enum FsPathInner {
    /// absolute path
    Absolute(String),
    /// path is relative to cwd
    CwdRelative(String),
    /// cwd
    Cwd,
    /// path is relative to dir fd
    FdRelative(FileDesc, String),
    /// fd itself
    Fd(FileDesc),
}

impl FsPathInner {
    pub fn new(path: String, dirfd: i32) -> Result<Self> {
        if path.len() > PATH_MAX {
            return_errno!(ENAMETOOLONG, "path name too long");
        }

        let fs_path_inner = if Path::new(&path).is_absolute() {
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
