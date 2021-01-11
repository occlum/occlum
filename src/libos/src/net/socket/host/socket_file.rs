use std::any::Any;
use std::io::{Read, Seek, SeekFrom, Write};

use atomic::{Atomic, Ordering};

use super::*;
use crate::fs::{
    occlum_ocall_ioctl, AccessMode, AtomicIoEvents, CreationFlags, File, FileRef, HostFd, IoEvents,
    IoctlCmd, StatusFlags,
};

//TODO: refactor write syscall to allow zero length with non-zero buffer
impl File for HostSocket {
    fn read(&self, buf: &mut [u8]) -> Result<usize> {
        self.recv(buf, RecvFlags::empty())
    }

    fn write(&self, buf: &[u8]) -> Result<usize> {
        self.send(buf, SendFlags::empty())
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
        let (bytes_recvd, _, _, _) = self.do_recvmsg(bufs, RecvFlags::empty(), None, None)?;
        Ok(bytes_recvd)
    }

    fn writev(&self, bufs: &[&[u8]]) -> Result<usize> {
        self.do_sendmsg(bufs, SendFlags::empty(), None, None)
    }

    fn seek(&self, pos: SeekFrom) -> Result<off_t> {
        return_errno!(ESPIPE, "Socket does not support seek")
    }

    fn ioctl(&self, cmd: &mut IoctlCmd) -> Result<i32> {
        self.ioctl_impl(cmd)
    }

    fn access_mode(&self) -> Result<AccessMode> {
        Ok(AccessMode::O_RDWR)
    }

    fn status_flags(&self) -> Result<StatusFlags> {
        let ret = try_libc!(libc::ocall::fcntl_arg0(
            self.raw_host_fd() as i32,
            libc::F_GETFL
        ));
        Ok(StatusFlags::from_bits_truncate(ret as u32))
    }

    fn set_status_flags(&self, new_status_flags: StatusFlags) -> Result<()> {
        let valid_flags_mask = StatusFlags::O_APPEND
            | StatusFlags::O_ASYNC
            | StatusFlags::O_DIRECT
            | StatusFlags::O_NOATIME
            | StatusFlags::O_NONBLOCK;
        let raw_status_flags = (new_status_flags & valid_flags_mask).bits();
        try_libc!(libc::ocall::fcntl_arg1(
            self.raw_host_fd() as i32,
            libc::F_SETFL,
            raw_status_flags as c_int
        ));
        Ok(())
    }

    fn poll_new(&self) -> IoEvents {
        self.host_events.load(Ordering::Acquire)
    }

    fn host_fd(&self) -> Option<&HostFd> {
        Some(&self.host_fd)
    }

    fn notifier(&self) -> Option<&IoNotifier> {
        Some(&self.notifier)
    }

    fn update_host_events(&self, ready: &IoEvents, mask: &IoEvents, trigger_notifier: bool) {
        self.host_events.update(ready, mask, Ordering::Release);

        if trigger_notifier {
            self.notifier.broadcast(ready);
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
