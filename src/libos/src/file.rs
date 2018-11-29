use prelude::*;
use {std};
use std::{fmt};

use std::sgxfs as fs_impl;
use std::io::{Read, Write, Seek, SeekFrom};

pub trait File : Debug + Sync + Send {
    fn read(&self, buf: &mut [u8]) -> Result<usize, Error>;
    fn write(&self, buf: &[u8]) -> Result<usize, Error>;
    fn readv<'a, 'b>(&self, bufs: &'a mut [&'b mut [u8]]) -> Result<usize, Error>;
    fn writev<'a, 'b>(&self, bufs: &'a [&'b [u8]]) -> Result<usize, Error>;
    //pub seek(&mut self, ) -> Result<usize, Error>;
}

pub type FileRef = Arc<Box<File>>;

#[derive(Debug)]
#[repr(C)]
pub struct SgxFile {
    inner: SgxMutex<SgxFileInner>,
}

impl SgxFile {
    pub fn new(file: Arc<SgxMutex<fs_impl::SgxFile>>,
               is_readable: bool, is_writable: bool, is_append: bool) -> SgxFile
    {
        SgxFile {
            inner: SgxMutex::new(SgxFileInner {
                pos: 0 as usize,
                file: file,
                is_readable,
                is_writable,
                is_append,
            }),
        }
    }
}

impl File for SgxFile {
    fn read(&self, buf: &mut [u8]) -> Result<usize, Error> {
        let mut inner_guard = self.inner.lock().unwrap();
        let inner = inner_guard.borrow_mut();
        inner.read(buf)
    }

    fn write(&self, buf: &[u8]) -> Result<usize, Error> {
        let mut inner_guard = self.inner.lock().unwrap();
        let inner = inner_guard.borrow_mut();
        inner.write(buf)
    }

    fn readv<'a, 'b>(&self, bufs: &'a mut [&'b mut [u8]]) -> Result<usize, Error> {
        let mut inner_guard = self.inner.lock().unwrap();
        let inner = inner_guard.borrow_mut();
        inner.readv(bufs)
    }

    fn writev<'a, 'b>(&self, bufs: &'a [&'b [u8]]) -> Result<usize, Error> {
        let mut inner_guard = self.inner.lock().unwrap();
        let inner = inner_guard.borrow_mut();
        inner.writev(bufs)
    }
}

#[derive(Clone)]
#[repr(C)]
struct SgxFileInner {
//    perms: FilePerms,
    pos: usize,
    file: Arc<SgxMutex<fs_impl::SgxFile>>,
    is_readable: bool,
    is_writable: bool,
    is_append: bool,
}

impl SgxFileInner {
    pub fn write(&mut self, buf: &[u8]) -> Result<usize, Error> {
        let mut file_guard = self.file.lock().unwrap();
        let file = file_guard.borrow_mut();

        let seek_pos = SeekFrom::Start(self.pos as u64);
        // TODO: recover from error
        file.seek(seek_pos).map_err(
            |e| Error::new(Errno::EINVAL, "Failed to seek to a position"))?;

        let write_len = {
            file.write(buf).map_err(
                |e| Error::new(Errno::EINVAL, "Failed to write"))?
        };

        self.pos += write_len;
        Ok(write_len)
    }

    pub fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
        let mut file_guard = self.file.lock().unwrap();
        let file = file_guard.borrow_mut();

        let seek_pos = SeekFrom::Start(self.pos as u64);
        file.seek(seek_pos).map_err(
            |e| Error::new(Errno::EINVAL, "Failed to seek to a position"))?;

        let read_len = {
            file.read(buf).map_err(
                |e| Error::new(Errno::EINVAL, "Failed to write"))?
        };

        self.pos += read_len;
        Ok(read_len)
    }

    pub fn writev<'a, 'b>(&mut self, bufs: &'a [&'b [u8]]) -> Result<usize, Error> {
        let mut file_guard = self.file.lock().unwrap();
        let file = file_guard.borrow_mut();

        let seek_pos = SeekFrom::Start(self.pos as u64);
        file.seek(seek_pos).map_err(
            |e| Error::new(Errno::EINVAL, "Failed to seek to a position"))?;

        let mut total_bytes = 0;
        for buf in bufs {
            match file.write(buf) {
                Ok(this_bytes) => {
                    if this_bytes == 0 { break; }

                    total_bytes += this_bytes;
                    if this_bytes < buf.len() { break; }
                }
                Err(e) => {
                    match total_bytes {
                        // a complete failure
                        0 => return Err(Error::new(Errno::EINVAL, "Failed to write")),
                        // a partially failure
                        _ => break,
                    }
                }
            }
        }

        self.pos += total_bytes;
        Ok(total_bytes)
    }

    fn readv<'a, 'b>(&mut self, bufs: &'a mut [&'b mut [u8]]) -> Result<usize, Error> {
        let mut file_guard = self.file.lock().unwrap();
        let file = file_guard.borrow_mut();

        let seek_pos = SeekFrom::Start(self.pos as u64);
        file.seek(seek_pos).map_err(
            |e| Error::new(Errno::EINVAL, "Failed to seek to a position"))?;

        let mut total_bytes = 0;
        for buf in bufs {
            match file.read(buf) {
                Ok(this_bytes) => {
                    if this_bytes == 0 { break; }

                    total_bytes += this_bytes;
                    if this_bytes < buf.len() { break; }
                }
                Err(e) => {
                    match total_bytes {
                        // a complete failure
                        0 => return Err(Error::new(Errno::EINVAL, "Failed to write")),
                        // a partially failure
                        _ => break,
                    }
                }
            }
        }

        self.pos += total_bytes;
        Ok(total_bytes)
    }
}

unsafe impl Send for SgxFileInner {}
unsafe impl Sync for SgxFileInner {}

impl Debug for SgxFileInner {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "SgxFileInner {{ pos: {}, file: ??? }}", self.pos)
    }
}

pub struct StdoutFile {
    inner: std::io::Stdout,
}

impl StdoutFile {
    pub fn new() -> StdoutFile {
        StdoutFile {
            inner: std::io::stdout(),
        }
    }
}

impl File for StdoutFile {
    fn write(&self, buf: &[u8]) -> Result<usize, Error> {
        let write_len = {
            self.inner.lock().write(buf).map_err(|e| (Errno::EINVAL,
                                           "Failed to write"))?
        };
        Ok(write_len)
    }

    fn read(&self, buf: &mut [u8]) -> Result<usize, Error> {
        Err(Error::new(Errno::EBADF, "Stdout does not support reading"))
    }

    fn writev<'a, 'b>(&self, bufs: &'a [&'b [u8]]) -> Result<usize, Error> {
        let mut guard = self.inner.lock();
        let mut total_bytes = 0;
        for buf in bufs {
            match guard.write(buf) {
                Ok(this_len) => {
                    if this_len == 0 { break; }
                    total_bytes += this_len;
                    if this_len < buf.len() { break; }
                }
                Err(e) => {
                    match total_bytes {
                        // a complete failure
                        0 => return Err(Error::new(Errno::EINVAL, "Failed to write")),
                        // a partially failure
                        _ => break,
                    }
                }
            }
        }
        Ok(total_bytes)
    }

    fn readv<'a, 'b>(&self, bufs: &'a mut [&'b mut [u8]]) -> Result<usize, Error> {
        Err(Error::new(Errno::EBADF, "Stdout does not support reading"))
    }
}

impl Debug for StdoutFile {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "StdoutFile")
    }
}

unsafe impl Send for StdoutFile {}
unsafe impl Sync for StdoutFile {}

pub struct StdinFile {
    inner: std::io::Stdin,
}

impl StdinFile {
    pub fn new() -> StdinFile {
        StdinFile {
            inner: std::io::stdin(),
        }
    }
}

impl File for StdinFile {
    fn read(&self, buf: &mut [u8]) -> Result<usize, Error> {
        let read_len = {
            self.inner.lock().read(buf).map_err(|e| (Errno::EINVAL,
                                           "Failed to read"))?
        };
        Ok(read_len)
    }

    fn write(&self, buf: &[u8]) -> Result<usize, Error> {
        Err(Error::new(Errno::EBADF, "Stdin does not support reading"))
    }

    fn readv<'a, 'b>(&self, bufs: &'a mut [&'b mut [u8]]) -> Result<usize, Error> {
        let mut guard = self.inner.lock();
        let mut total_bytes = 0;
        for buf in bufs {
            match guard.read(buf) {
                Ok(this_len) => {
                    if this_len == 0 { break; }
                    total_bytes += this_len;
                    if this_len < buf.len() { break; }
                }
                Err(e) => {
                    match total_bytes {
                        // a complete failure
                        0 => return Err(Error::new(Errno::EINVAL, "Failed to write")),
                        // a partially failure
                        _ => break,
                    }
                }
            }
        }
        Ok(total_bytes)
    }

    fn writev<'a, 'b>(&self, bufs: &'a [&'b [u8]]) -> Result<usize, Error> {
        Err(Error::new(Errno::EBADF, "Stdin does not support reading"))
    }
}

impl Debug for StdinFile {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "StdinFile")
    }
}

unsafe impl Send for StdinFile {}
unsafe impl Sync for StdinFile {}
