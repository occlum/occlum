use super::address_space::ADDRESS_SPACE;
use super::stream::Status;
use super::*;
use fs::{AccessMode, File, IoEvents, IoNotifier, StatusFlags};
use rcore_fs::vfs::{FileType, Metadata, Timespec};
use std::any::Any;

use crate::fs::{GetReadBufLen, IoctlCmd, SetNonBlocking};

impl File for Stream {
    fn read(&self, buf: &mut [u8]) -> Result<usize> {
        // The connected status will not be changed any more
        // in the current implementation. Use clone to release
        // the mutex lock early.
        let status = (*self.inner()).clone();
        match status {
            Status::Connected(endpoint) => endpoint.read(buf),
            _ => return_errno!(ENOTCONN, "unconnected socket"),
        }
    }

    fn write(&self, buf: &[u8]) -> Result<usize> {
        let status = (*self.inner()).clone();
        match status {
            Status::Connected(endpoint) => endpoint.write(buf),
            _ => return_errno!(ENOTCONN, "unconnected socket"),
        }
    }

    fn read_at(&self, offset: usize, buf: &mut [u8]) -> Result<usize> {
        if offset != 0 {
            return_errno!(ESPIPE, "a nonzero position is not supported");
        }
        self.read(buf)
    }

    fn write_at(&self, offset: usize, buf: &[u8]) -> Result<usize> {
        if offset != 0 {
            return_errno!(ESPIPE, "a nonzero position is not supported");
        }
        self.write(buf)
    }

    fn readv(&self, bufs: &mut [&mut [u8]]) -> Result<usize> {
        let status = (*self.inner()).clone();
        match status {
            Status::Connected(endpoint) => endpoint.readv(bufs),
            _ => return_errno!(ENOTCONN, "unconnected socket"),
        }
    }

    fn writev(&self, bufs: &[&[u8]]) -> Result<usize> {
        let status = (*self.inner()).clone();
        match status {
            Status::Connected(endpoint) => endpoint.writev(bufs),
            _ => return_errno!(ENOTCONN, "unconnected socket"),
        }
    }

    fn ioctl(&self, cmd: &mut dyn IoctlCmd) -> Result<()> {
        match_ioctl_cmd_auto_error!(cmd, {
            cmd : GetReadBufLen => {
                match &*self.inner() {
                    Status::Connected(endpoint) => {
                        let bytes_to_read = endpoint.bytes_to_read().min(std::i32::MAX as usize) as i32;
                        cmd.set_output(bytes_to_read as _);
                    }
                    _ => return_errno!(ENOTCONN, "unconnected socket"),
                };
            },
            cmd : SetNonBlocking => {
                let nonblocking = cmd.input();
                self.set_nonblocking(*nonblocking != 0);
            }
        });
        Ok(())
    }

    fn access_mode(&self) -> Result<AccessMode> {
        Ok(AccessMode::O_RDWR)
    }

    fn status_flags(&self) -> Result<StatusFlags> {
        if self.nonblocking() {
            Ok(StatusFlags::O_NONBLOCK)
        } else {
            Ok(StatusFlags::empty())
        }
    }

    fn set_status_flags(&self, new_status_flags: StatusFlags) -> Result<()> {
        // Only O_NONBLOCK, O_ASYNC and O_DIRECT can be set
        let status_flags = new_status_flags
            & (StatusFlags::O_NONBLOCK | StatusFlags::O_ASYNC | StatusFlags::O_DIRECT);

        // Only O_NONBLOCK is supported
        let nonblocking = new_status_flags.contains(StatusFlags::O_NONBLOCK);
        self.set_nonblocking(nonblocking);
        Ok(())
    }

    fn poll_new(&self) -> IoEvents {
        match &*self.inner() {
            // linux return value
            Status::Idle(info) => IoEvents::OUT | IoEvents::HUP,
            Status::Connected(endpoint) => endpoint.poll(),
            Status::Listening(addr) => {
                if let Some(listener) = ADDRESS_SPACE.get_listener_ref(addr) {
                    listener.poll_new()
                } else {
                    IoEvents::empty()
                }
            }
        }
    }

    fn notifier(&self) -> Option<&IoNotifier> {
        Some(&self.notifier.notifier())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn metadata(&self) -> Result<Metadata> {
        Ok(Metadata {
            dev: 0,
            inode: 0,
            size: 0,
            blk_size: 0,
            blocks: 0,
            atime: Timespec { sec: 0, nsec: 0 },
            mtime: Timespec { sec: 0, nsec: 0 },
            ctime: Timespec { sec: 0, nsec: 0 },
            type_: FileType::Socket,
            mode: 0o666,
            nlinks: 1,
            uid: 0,
            gid: 0,
            rdev: 0,
        })
    }
}
