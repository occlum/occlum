use block_device::{BlockDevice, BlockDeviceExt, BLOCK_SIZE};
use std::fmt;

use crate::fs::{
    AccessMode, Events, FileType, IoctlCmd, Metadata, Observer, Pollee, Poller, SeekFrom,
    StatusFlags, Timespec,
};
use crate::prelude::*;

/// A file wrapper for a block device.
pub struct DiskFile {
    disk: Arc<dyn BlockDevice>,
    // TODO: use async lock
    offset: SgxMutex<usize>,
}

impl DiskFile {
    pub fn new(disk: Arc<dyn BlockDevice>) -> Self {
        Self {
            disk,
            offset: SgxMutex::new(0),
        }
    }

    pub fn register_observer(&self, _observer: Arc<dyn Observer>, _mask: Events) -> Result<()> {
        return_errno!(EINVAL, "disk files do not support observers");
    }

    pub fn unregister_observer(&self, _observer: &Arc<dyn Observer>) -> Result<Arc<dyn Observer>> {
        return_errno!(EINVAL, "disk files do not support observers");
    }

    pub async fn read(&self, buf: &mut [u8]) -> Result<usize> {
        let mut offset = self.offset.lock().unwrap();
        let len = self.disk.read(*offset, buf).await?;
        *offset += len;
        Ok(len)
    }

    pub async fn readv(&self, bufs: &mut [&mut [u8]]) -> Result<usize> {
        let mut offset = self.offset.lock().unwrap();
        let mut total_len = 0;
        for buf in bufs {
            match self.disk.read(*offset, buf).await {
                Ok(len) => {
                    total_len += len;
                    if len < buf.len() {
                        break;
                    }
                    *offset += len;
                }
                Err(_) if total_len != 0 => break,
                Err(e) => return Err(e),
            }
        }
        Ok(total_len)
    }

    pub async fn write(&self, buf: &[u8]) -> Result<usize> {
        let mut offset = self.offset.lock().unwrap();
        let len = self.disk.write(*offset, buf).await?;
        *offset += len;
        Ok(len)
    }

    pub async fn writev(&self, bufs: &[&[u8]]) -> Result<usize> {
        let mut offset = self.offset.lock().unwrap();
        let mut total_len = 0;
        for buf in bufs {
            match self.disk.write(*offset, buf).await {
                Ok(len) => {
                    total_len += len;
                    if len < buf.len() {
                        break;
                    }
                    *offset += len;
                }
                Err(_) if total_len != 0 => break,
                Err(e) => return Err(e),
            }
        }
        Ok(total_len)
    }

    pub async fn flush(&self) -> Result<()> {
        self.disk.flush().await
    }

    pub fn seek(&self, pos: SeekFrom) -> Result<usize> {
        let mut offset = self.offset.lock().unwrap();
        let new_offset: i64 = match pos {
            SeekFrom::Start(off /* as u64 */) => {
                if off > i64::max_value() as u64 {
                    return_errno!(EINVAL, "file offset is too large");
                }
                off as i64
            }
            SeekFrom::End(off /* as u64 */) => {
                let file_size = self.disk.total_bytes() as i64;
                assert!(file_size >= 0);
                file_size
                    .checked_add(off)
                    .ok_or_else(|| errno!(EOVERFLOW, "file offset overflow"))?
            }
            SeekFrom::Current(off /* as i64 */) => (*offset as i64)
                .checked_add(off)
                .ok_or_else(|| errno!(EOVERFLOW, "file offset overflow"))?,
        };
        if new_offset < 0 {
            return_errno!(EINVAL, "file offset must not be negative");
        }
        // Invariant: 0 <= new_offset <= i64::max_value()
        let new_offset = new_offset as usize;
        *offset = new_offset;
        Ok(new_offset)
    }

    pub fn poll(&self, _mask: Events, _poller: Option<&mut Poller>) -> Events {
        Events::IN | Events::OUT
    }

    pub fn access_mode(&self) -> AccessMode {
        AccessMode::O_RDWR
    }

    pub fn status_flags(&self) -> StatusFlags {
        StatusFlags::empty()
    }

    pub fn set_status_flags(&self, _new_status: StatusFlags) -> Result<()> {
        return_errno!(ENOSYS, "not support setting status flags");
    }

    pub fn ioctl(&self, _cmd: &mut dyn IoctlCmd) -> Result<()> {
        return_errno!(EINVAL, "this file does not support ioctl");
    }

    pub fn metadata(&self) -> Metadata {
        Metadata {
            dev: 0,
            // Use a large number to avoid to coincide with a valid inode number.
            inode: 0xfe23_1d08,
            size: self.disk.total_bytes(),
            blk_size: BLOCK_SIZE,
            blocks: self.disk.total_bytes() / 512,
            atime: Timespec { sec: 0, nsec: 0 },
            mtime: Timespec { sec: 0, nsec: 0 },
            ctime: Timespec { sec: 0, nsec: 0 },
            // FIO will access FileType::BlockDevice with some raw ioctls.
            // To test the R/W speed of blockdevice as normal file, return FileType::File.
            type_: FileType::File,
            mode: 0o666,
            nlinks: 1,
            uid: 0,
            gid: 0,
            rdev: 0,
        }
    }
}

impl Debug for DiskFile {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "DiskFile {{ offset: {} }}", *self.offset.lock().unwrap())
    }
}
