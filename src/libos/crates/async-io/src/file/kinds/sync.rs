use std::fmt::Debug;

use rcore_fs::vfs::INode;

use crate::file::{AccessMode, SeekFrom, StatusFlags};
use crate::poll::Events;
use crate::prelude::*;

pub trait SyncFile: Debug + Sync + Send {
    fn read(&self, _buf: &mut [u8]) -> Result<usize> {
        return_errno!(EBADF, "not support read");
    }

    fn readv(&self, bufs: &mut [&mut [u8]]) -> Result<usize> {
        for buf in bufs {
            if buf.len() > 0 {
                return self.read(buf);
            }
        }
        Ok(0)
    }

    fn read_at(&self, _offset: usize, buf: &mut [u8]) -> Result<usize> {
        return_errno!(ESPIPE, "not support seek or read");
    }

    fn write(&self, _buf: &[u8]) -> Result<usize> {
        return_errno!(EBADF, "not support write");
    }

    fn writev(&self, bufs: &[&[u8]]) -> Result<usize> {
        for buf in bufs {
            if buf.len() > 0 {
                return self.write(buf);
            }
        }
        Ok(0)
    }

    fn write_at(&self, _offset: usize, buf: &[u8]) -> Result<usize> {
        return_errno!(ESPIPE, "not support seek or write");
    }

    fn flush(&self) -> Result<()> {
        Ok(())
    }

    fn seek(&self, _pos: SeekFrom) -> Result<usize> {
        return_errno!(ESPIPE, "not support seek");
    }

    fn poll(&self, mask: Events) -> Events;

    /*
        fn ioctl(&self, cmd: &mut IoctlCmd) -> Result<i32> {
            return_op_unsupported_error!("ioctl")
        }
    */

    fn access_mode(&self) -> Result<AccessMode> {
        return_errno!(ENOSYS, "not support getting access mode");
    }

    fn status_flags(&self) -> Result<StatusFlags> {
        return_errno!(ENOSYS, "not support getting status flags");
    }

    fn set_status_flags(&self, new_status: StatusFlags) -> Result<()> {
        return_errno!(ENOSYS, "not support setting status flags");
    }

    fn as_inode(&self) -> Option<&dyn INode> {
        None
    }
}
