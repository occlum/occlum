use std::fmt;
use std::sync::Arc;

use rcore_fs::vfs::INode;

use crate::file::{AccessMode, Async, PollableFile, SeekFrom, StatusFlags, SyncFile};
use crate::poll::{Events, Poller};
use crate::prelude::*;

/// File handles providing a unified, async file interface regardless of the underlying
/// implemention of the file type.
pub enum FileHandle {
    // For file types that support polling I/O, e.g., sockets, pipe, event_fd, etc.
    // These files can be easily made async with poller/pollee API.
    Pollable(Async<Arc<dyn PollableFile>>),
    // For file types that only support sync I/O APIs, e.g., inode types that are
    // from the rcore-fs.
    Sync(Arc<dyn SyncFile>),
    // For inode types that support boxed-based async APIs.
    //Async(Arc<dyn AsyncFile>),
}

impl FileHandle {
    pub fn from_pollable(file: Arc<dyn PollableFile>) -> Self {
        FileHandle::Pollable(Async::new(file))
    }

    pub fn from_sync(file: Arc<dyn SyncFile>) -> Self {
        FileHandle::Sync(file)
    }

    pub async fn read(&self, buf: &mut [u8]) -> Result<usize> {
        match self {
            Self::Pollable(f) => f.read(buf).await,
            Self::Sync(f) => f.read(buf),
        }
    }

    pub async fn readv(&self, bufs: &mut [&mut [u8]]) -> Result<usize> {
        match self {
            Self::Pollable(f) => f.readv(bufs).await,
            Self::Sync(f) => f.readv(bufs),
        }
    }

    pub async fn read_at(&self, offset: usize, buf: &mut [u8]) -> Result<usize> {
        match self {
            Self::Pollable(f) => f.read_at(offset, buf).await,
            Self::Sync(f) => f.read_at(offset, buf),
        }
    }

    pub async fn read_exact(&self, buf: &mut [u8]) -> Result<()> {
        let mut count = 0;
        while count < buf.len() {
            // TODO: handle EINTR
            let nbytes = self.read(&mut buf[count..]).await?;
            if nbytes == 0 {
                return_errno!(EINVAL, "unexpected EOF");
            }
            count += nbytes;
        }
        Ok(())
    }

    pub async fn read_exact_at(&self, offset: usize, buf: &mut [u8]) -> Result<()> {
        let mut count = 0;
        while count < buf.len() {
            // TODO: handle EINTR
            let nbytes = self.read_at(offset + count, &mut buf[count..]).await?;
            if nbytes == 0 {
                return_errno!(EINVAL, "unexpected EOF");
            }
            count += nbytes;
        }
        Ok(())
    }

    pub async fn write(&self, buf: &[u8]) -> Result<usize> {
        match self {
            Self::Pollable(f) => f.write(buf).await,
            Self::Sync(f) => f.write(buf),
        }
    }

    pub async fn writev(&self, bufs: &[&[u8]]) -> Result<usize> {
        match self {
            Self::Pollable(f) => f.writev(bufs).await,
            Self::Sync(f) => f.writev(bufs),
        }
    }

    pub async fn write_at(&self, offset: usize, buf: &[u8]) -> Result<usize> {
        match self {
            Self::Pollable(f) => f.write_at(offset, buf).await,
            Self::Sync(f) => f.write_at(offset, buf),
        }
    }

    pub async fn write_exact(&self, buf: &[u8]) -> Result<()> {
        let mut count = 0;
        while count < buf.len() {
            // TODO: Handle EINTR
            count += self.write(&buf[count..]).await?;
        }
        Ok(())
    }

    pub async fn write_exact_at(&self, offset: usize, buf: &[u8]) -> Result<()> {
        let mut count = 0;
        while count < buf.len() {
            // TODO: Handle EINTR
            count += self.write_at(offset + count, &buf[count..]).await?;
        }
        Ok(())
    }

    pub async fn flush(&self) -> Result<()> {
        match self {
            Self::Pollable(f) => f.flush().await,
            Self::Sync(f) => f.flush(),
        }
    }

    pub fn seek(&self, pos: SeekFrom) -> Result<usize> {
        match self {
            Self::Pollable(f) => f.seek(pos),
            Self::Sync(f) => f.seek(pos),
        }
    }

    pub fn poll_by(&self, mask: Events, poller: Option<&mut Poller>) -> Events {
        match self {
            Self::Pollable(f) => f.poll_by(mask, poller),
            Self::Sync(f) => f.poll(mask),
        }
    }

    /*
        fn ioctl(&self, cmd: &mut IoctlCmd) -> Result<i32> {
            return_op_unsupported_error!("ioctl")
        }
    */

    pub fn access_mode(&self) -> Result<AccessMode> {
        match self {
            Self::Pollable(f) => f.access_mode(),
            Self::Sync(f) => f.access_mode(),
        }
    }

    pub fn status_flags(&self) -> Result<StatusFlags> {
        match self {
            Self::Pollable(f) => f.status_flags(),
            Self::Sync(f) => f.status_flags(),
        }
    }

    pub fn set_status_flags(&self, new_status: StatusFlags) -> Result<()> {
        match self {
            Self::Pollable(f) => f.set_status_flags(new_status),
            Self::Sync(f) => f.set_status_flags(new_status),
        }
    }

    pub fn as_inode(&self) -> Option<&dyn INode> {
        match self {
            Self::Pollable(f) => None,
            Self::Sync(f) => f.as_inode(),
        }
    }
}

impl Clone for FileHandle {
    fn clone(&self) -> Self {
        match self {
            Self::Pollable(f) => Self::Pollable(f.clone()),
            Self::Sync(f) => Self::Sync(f.clone()),
        }
    }
}

impl PartialEq for FileHandle {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Pollable(self_f), Self::Pollable(other_f)) => {
                let self_file_arc = self_f.file();
                let other_file_arc = other_f.file();
                Arc::ptr_eq(self_file_arc, other_file_arc)
            }
            (Self::Sync(self_f), Self::Sync(other_f)) => Arc::ptr_eq(self_f, other_f),
            _ => false,
        }
    }
}

impl Eq for FileHandle {}

impl fmt::Debug for FileHandle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pollable(file) => formatter
                .debug_struct("FileHandle::Pollable")
                .field("inner", file)
                .finish(),
            Self::Sync(file) => formatter
                .debug_struct("FileHandle::Sync")
                .field("inner", file)
                .finish(),
        }
    }
}
