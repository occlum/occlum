use super::*;
use std::any::Any;

/// Native Linux socket
#[derive(Debug)]
pub struct SocketFile {
    fd: c_int,
}

impl SocketFile {
    pub fn new(domain: c_int, socket_type: c_int, protocol: c_int) -> Result<Self, Error> {
        let ret = try_libc!(libc::ocall::socket(domain, socket_type, protocol));
        Ok(SocketFile { fd: ret })
    }

    pub fn accept(
        &self,
        addr: *mut libc::sockaddr,
        addr_len: *mut libc::socklen_t,
        flags: c_int,
    ) -> Result<Self, Error> {
        let ret = try_libc!(libc::ocall::accept4(self.fd, addr, addr_len, flags));
        Ok(SocketFile { fd: ret })
    }

    pub fn fd(&self) -> c_int {
        self.fd
    }
}

impl Drop for SocketFile {
    fn drop(&mut self) {
        let ret = unsafe { libc::ocall::close(self.fd) };
        if ret < 0 {
            let errno = unsafe { libc::errno() };
            warn!(
                "socket (host fd: {}) close failed: errno = {}",
                self.fd, errno
            );
        }
    }
}

impl File for SocketFile {
    fn read(&self, buf: &mut [u8]) -> Result<usize, Error> {
        let ret = try_libc!(libc::ocall::read(
            self.fd,
            buf.as_mut_ptr() as *mut c_void,
            buf.len()
        ));
        Ok(ret as usize)
    }

    fn write(&self, buf: &[u8]) -> Result<usize, Error> {
        let ret = try_libc!(libc::ocall::write(
            self.fd,
            buf.as_ptr() as *const c_void,
            buf.len()
        ));
        Ok(ret as usize)
    }

    fn read_at(&self, _offset: usize, buf: &mut [u8]) -> Result<usize, Error> {
        self.read(buf)
    }

    fn write_at(&self, _offset: usize, buf: &[u8]) -> Result<usize, Error> {
        self.write(buf)
    }

    fn readv(&self, bufs: &mut [&mut [u8]]) -> Result<usize, Error> {
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

    fn writev(&self, bufs: &[&[u8]]) -> Result<usize, Error> {
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

    fn seek(&self, pos: SeekFrom) -> Result<off_t, Error> {
        errno!(ESPIPE, "Socket does not support seek")
    }

    fn metadata(&self) -> Result<Metadata, Error> {
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
            mode: 0,
            nlinks: 0,
            uid: 0,
            gid: 0,
        })
    }

    fn set_len(&self, len: u64) -> Result<(), Error> {
        unimplemented!()
    }

    fn sync_all(&self) -> Result<(), Error> {
        unimplemented!()
    }

    fn sync_data(&self) -> Result<(), Error> {
        unimplemented!()
    }

    fn read_entry(&self) -> Result<String, Error> {
        unimplemented!()
    }

    fn as_any(&self) -> &Any {
        self
    }
}

pub trait AsSocket {
    fn as_socket(&self) -> Result<&SocketFile, Error>;
}

impl AsSocket for FileRef {
    fn as_socket(&self) -> Result<&SocketFile, Error> {
        self.as_any()
            .downcast_ref::<SocketFile>()
            .ok_or(Error::new(Errno::EBADF, "not a socket"))
    }
}
