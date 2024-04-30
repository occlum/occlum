use super::socket_file::SocketFile;
use crate::fs::{
    AccessMode, FileDesc, HostFd, IoEvents, IoNotifier, IoctlCmd, IoctlRawCmd, StatusFlags,
};
use crate::prelude::*;
use std::{io::SeekFrom, os::unix::raw::off_t};

impl File for SocketFile {
    fn read(&self, buf: &mut [u8]) -> Result<usize> {
        self.read(buf)
    }

    fn readv(&self, bufs: &mut [&mut [u8]]) -> Result<usize> {
        self.readv(bufs)
    }

    fn write(&self, buf: &[u8]) -> Result<usize> {
        self.write(buf)
    }

    fn writev(&self, bufs: &[&[u8]]) -> Result<usize> {
        self.writev(bufs)
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

    fn seek(&self, pos: SeekFrom) -> Result<off_t> {
        return_errno!(ESPIPE, "Socket does not support seek")
    }

    fn ioctl(&self, cmd: &mut dyn IoctlCmd) -> Result<()> {
        self.ioctl(cmd)
    }

    fn notifier(&self) -> Option<&IoNotifier> {
        Some(self.notifier())
    }

    fn access_mode(&self) -> Result<AccessMode> {
        Ok(AccessMode::O_RDWR)
    }

    fn status_flags(&self) -> Result<StatusFlags> {
        Ok(self.status_flags())
    }

    fn set_status_flags(&self, new_status_flags: StatusFlags) -> Result<()> {
        self.set_status_flags(new_status_flags)
    }

    fn poll_new(&self) -> IoEvents {
        let mask = IoEvents::all();
        self.poll(mask, None)
    }

    fn host_fd(&self) -> Option<&HostFd> {
        None
    }

    fn update_host_events(&self, ready: &IoEvents, mask: &IoEvents, trigger_notifier: bool) {
        unreachable!()
    }

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
}
