use super::*;

mod recv;
mod send;

use fs::{AccessMode, CreationFlags, File, FileRef, IoctlCmd, StatusFlags};
use std::any::Any;
use std::io::{Read, Seek, SeekFrom, Write};

/// Native Linux socket
#[derive(Debug)]
pub struct SocketFile {
    host_fd: c_int,
}

impl SocketFile {
    pub fn new(domain: c_int, socket_type: c_int, protocol: c_int) -> Result<Self> {
        let ret = try_libc!(libc::ocall::socket(domain, socket_type, protocol));
        Ok(SocketFile { host_fd: ret })
    }

    pub fn accept(
        &self,
        addr: *mut libc::sockaddr,
        addr_len: *mut libc::socklen_t,
        flags: c_int,
    ) -> Result<Self> {
        let ret = try_libc!(libc::ocall::accept4(self.host_fd, addr, addr_len, flags));
        Ok(SocketFile { host_fd: ret })
    }

    pub fn fd(&self) -> c_int {
        self.host_fd
    }
}

impl Drop for SocketFile {
    fn drop(&mut self) {
        let ret = unsafe { libc::ocall::close(self.host_fd) };
        assert!(ret == 0);
    }
}

// TODO: rewrite read/write/readv/writev as send/recv
// TODO: implement readfrom/sendto
impl File for SocketFile {
    fn read(&self, buf: &mut [u8]) -> Result<usize> {
        let ret = try_libc!(libc::ocall::read(
            self.host_fd,
            buf.as_mut_ptr() as *mut c_void,
            buf.len()
        ));
        Ok(ret as usize)
    }

    fn write(&self, buf: &[u8]) -> Result<usize> {
        let ret = try_libc!(libc::ocall::write(
            self.host_fd,
            buf.as_ptr() as *const c_void,
            buf.len()
        ));
        Ok(ret as usize)
    }

    fn read_at(&self, _offset: usize, buf: &mut [u8]) -> Result<usize> {
        self.read(buf)
    }

    fn write_at(&self, _offset: usize, buf: &[u8]) -> Result<usize> {
        self.write(buf)
    }

    fn readv(&self, bufs: &mut [&mut [u8]]) -> Result<usize> {
        let mut total_len = 0;
        for buf in bufs {
            match self.read(buf) {
                Ok(len) => {
                    total_len += len;
                }
                Err(_) if total_len != 0 => break,
                Err(e) => return Err(e.into()),
            }
        }
        Ok(total_len)
    }

    fn writev(&self, bufs: &[&[u8]]) -> Result<usize> {
        let mut total_len = 0;
        for buf in bufs {
            match self.write(buf) {
                Ok(len) => {
                    total_len += len;
                }
                Err(_) if total_len != 0 => break,
                Err(e) => return Err(e.into()),
            }
        }
        Ok(total_len)
    }

    fn seek(&self, pos: SeekFrom) -> Result<off_t> {
        return_errno!(ESPIPE, "Socket does not support seek")
    }

    fn ioctl(&self, cmd: &mut IoctlCmd) -> Result<()> {
        let cmd_num = cmd.cmd_num() as c_int;
        let cmd_arg_ptr = cmd.arg_ptr() as *const c_int;
        try_libc!(libc::ocall::ioctl_arg1(self.fd(), cmd_num, cmd_arg_ptr));
        // FIXME: add sanity checks for results returned for socket-related ioctls
        cmd.validate_arg_val()?;
        Ok(())
    }

    fn get_access_mode(&self) -> Result<AccessMode> {
        Ok(AccessMode::O_RDWR)
    }

    fn get_status_flags(&self) -> Result<StatusFlags> {
        let ret = try_libc!(libc::ocall::fcntl_arg0(self.fd(), libc::F_GETFL));
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
            self.fd(),
            libc::F_SETFL,
            raw_status_flags as c_int
        ));
        Ok(())
    }

    fn as_any(&self) -> &Any {
        self
    }
}

pub trait AsSocket {
    fn as_socket(&self) -> Result<&SocketFile>;
}

impl AsSocket for FileRef {
    fn as_socket(&self) -> Result<&SocketFile> {
        self.as_any()
            .downcast_ref::<SocketFile>()
            .ok_or_else(|| errno!(EBADF, "not a socket"))
    }
}
