//! Raw disk.
use super::{Bid, BlockDevice};
use crate::blk::BLOCK_SIZE;
use crate::prelude::*;

use alloc::ffi::CString;
use alloc::string::{String, ToString};
use core::ops::Range;
use sgx_trts::libc::ocall::{fdatasync, ftruncate, open64, pread64, pwrite64, unlink};
use sgx_trts::libc::{O_CREAT, O_DIRECT, O_RDWR, O_TRUNC};
use std::os::unix::io::{AsRawFd, RawFd};

use ext2_rs::FsError as Ext2Error;
use sworndisk_v2::{BlockId, BlockSet, BufMut, BufRef, Errno as SwornErrno, Error as SwornError};

/// A raw disk as a block device, backed by a host file.
#[derive(Clone, Debug)]
pub struct RawDisk {
    fd: RawFd,
    path: String,
    range: Range<BlockId>,
}

impl RawDisk {
    pub fn open_or_create(nblocks: usize, path: &str) -> Result<Self> {
        unsafe {
            let flags = O_RDWR | O_CREAT; // w/o O_DIRECT
                                          // let flags = O_RDWR | O_CREAT | O_DIRECT; // w/o O_TRUNC
            let cpath = CString::new(path).unwrap();
            let fd = open64(cpath.as_ptr() as _, flags, 0o666);
            if fd == -1 {
                return_errno!(EIO, "raw disk open failed");
            }

            let res = ftruncate(fd, (nblocks * BLOCK_SIZE) as _);
            if res == -1 {
                return_errno!(EIO, "raw disk truncate failed");
            }

            Ok(Self {
                fd,
                path: path.to_string(),
                range: 0..nblocks,
            })
        }
    }
}

// Used by `SwornDisk` as its underlying disk.
impl BlockSet for RawDisk {
    fn read(&self, mut pos: BlockId, mut buf: BufMut) -> core::result::Result<(), SwornError> {
        pos += self.range.start;
        debug_assert!(pos + buf.nblocks() <= self.range.end);

        let buf_mut_slice = buf.as_mut_slice();
        unsafe {
            let res = pread64(
                self.fd,
                buf_mut_slice.as_ptr() as _,
                buf_mut_slice.len(),
                (pos * BLOCK_SIZE) as _,
            );
            if res == -1 {
                return Err(SwornError::with_msg(
                    SwornErrno::IoFailed,
                    "raw disk read failed",
                ));
            }
        }

        Ok(())
    }

    fn write(&self, mut pos: BlockId, buf: BufRef) -> core::result::Result<(), SwornError> {
        pos += self.range.start;
        debug_assert!(pos + buf.nblocks() <= self.range.end);

        let buf_slice = buf.as_slice();
        unsafe {
            let res = pwrite64(
                self.fd,
                buf_slice.as_ptr() as _,
                buf_slice.len(),
                (pos * BLOCK_SIZE) as _,
            );
            if res == -1 {
                return Err(SwornError::with_msg(
                    SwornErrno::IoFailed,
                    "raw disk write failed",
                ));
            }
        }

        Ok(())
    }

    fn subset(&self, range: Range<BlockId>) -> core::result::Result<Self, SwornError>
    where
        Self: Sized,
    {
        debug_assert!(self.range.start + range.end <= self.range.end);
        Ok(Self {
            fd: self.fd,
            path: self.path.clone(),
            range: Range {
                start: self.range.start + range.start,
                end: self.range.start + range.end,
            },
        })
    }

    fn flush(&self) -> core::result::Result<(), SwornError> {
        unsafe {
            let res = fdatasync(self.fd);
            if res == -1 {
                return Err(SwornError::with_msg(
                    SwornErrno::IoFailed,
                    "raw disk sync failed",
                ));
            }
        }
        Ok(())
    }

    fn nblocks(&self) -> usize {
        self.range.len()
    }
}

impl Drop for RawDisk {
    fn drop(&mut self) {
        unsafe {
            // XXX: When should we delete the host file?
            // unlink(self.path.as_ptr() as _);
        }
    }
}
