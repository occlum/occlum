use super::stream::Status;
use super::*;
use fs::{AccessMode, File, FileRef, IoctlCmd, StatusFlags};
use std::any::Any;

impl File for Stream {
    fn read(&self, buf: &mut [u8]) -> Result<usize> {
        match &*self.inner() {
            Status::Connected(endpoint) => endpoint.read(buf),
            _ => return_errno!(ENOTCONN, "unconnected socket"),
        }
    }

    fn write(&self, buf: &[u8]) -> Result<usize> {
        match &*self.inner() {
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
        match &*self.inner() {
            Status::Connected(endpoint) => endpoint.readv(bufs),
            _ => return_errno!(ENOTCONN, "unconnected socket"),
        }
    }

    fn writev(&self, bufs: &[&[u8]]) -> Result<usize> {
        match &*self.inner() {
            Status::Connected(endpoint) => endpoint.writev(bufs),
            _ => return_errno!(ENOTCONN, "unconnected socket"),
        }
    }

    fn ioctl(&self, cmd: &mut IoctlCmd) -> Result<i32> {
        match cmd {
            IoctlCmd::FIONREAD(arg) => match &*self.inner() {
                Status::Connected(endpoint) => {
                    let bytes_to_read = endpoint.bytes_to_read().min(std::i32::MAX as usize) as i32;
                    **arg = bytes_to_read;
                    Ok(0)
                }
                _ => return_errno!(ENOTCONN, "unconnected socket"),
            },
            _ => return_errno!(EINVAL, "unknown ioctl cmd for unix socket"),
        }
    }

    fn get_access_mode(&self) -> Result<AccessMode> {
        Ok(AccessMode::O_RDWR)
    }

    fn get_status_flags(&self) -> Result<StatusFlags> {
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

    fn poll(&self) -> Result<PollEventFlags> {
        warn!("poll is not supported for unix_socket");
        let events = PollEventFlags::empty();
        Ok(events)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
