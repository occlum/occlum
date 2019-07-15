use super::*;
use std;
use std::borrow::BorrowMut;
use std::fmt;
use std::io::SeekFrom;

pub trait File: Debug + Sync + Send + Any {
    fn read(&self, buf: &mut [u8]) -> Result<usize, Error>;
    fn write(&self, buf: &[u8]) -> Result<usize, Error>;
    fn read_at(&self, offset: usize, buf: &mut [u8]) -> Result<usize, Error>;
    fn write_at(&self, offset: usize, buf: &[u8]) -> Result<usize, Error>;
    fn readv(&self, bufs: &mut [&mut [u8]]) -> Result<usize, Error>;
    fn writev(&self, bufs: &[&[u8]]) -> Result<usize, Error>;
    fn seek(&self, pos: SeekFrom) -> Result<off_t, Error>;
    fn metadata(&self) -> Result<Metadata, Error>;
    fn set_len(&self, len: u64) -> Result<(), Error>;
    fn sync_all(&self) -> Result<(), Error>;
    fn sync_data(&self) -> Result<(), Error>;
    fn read_entry(&self) -> Result<String, Error>;
    fn as_any(&self) -> &Any;
}

pub type FileRef = Arc<Box<File>>;

#[derive(Debug)]
#[repr(C)]
pub struct SgxFile {
    inner: SgxMutex<SgxFileInner>,
}

impl SgxFile {
    pub fn new(
        file: Arc<SgxMutex<fs_impl::SgxFile>>,
        is_readable: bool,
        is_writable: bool,
        is_append: bool,
    ) -> Result<SgxFile, Error> {
        if !is_readable && !is_writable {
            return errno!(EINVAL, "Invalid permissions");
        }

        Ok(SgxFile {
            inner: SgxMutex::new(SgxFileInner {
                pos: 0 as usize,
                file: file,
                is_readable,
                is_writable,
                is_append,
            }),
        })
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

    fn read_at(&self, offset: usize, buf: &mut [u8]) -> Result<usize, Error> {
        let mut inner_guard = self.inner.lock().unwrap();
        let inner = inner_guard.borrow_mut();
        inner.seek(SeekFrom::Start(offset as u64))?;
        inner.read(buf)
    }

    fn write_at(&self, offset: usize, buf: &[u8]) -> Result<usize, Error> {
        let mut inner_guard = self.inner.lock().unwrap();
        let inner = inner_guard.borrow_mut();
        inner.seek(SeekFrom::Start(offset as u64))?;
        inner.write(buf)
    }

    fn readv(&self, bufs: &mut [&mut [u8]]) -> Result<usize, Error> {
        let mut inner_guard = self.inner.lock().unwrap();
        let inner = inner_guard.borrow_mut();
        inner.readv(bufs)
    }

    fn writev(&self, bufs: &[&[u8]]) -> Result<usize, Error> {
        let mut inner_guard = self.inner.lock().unwrap();
        let inner = inner_guard.borrow_mut();
        inner.writev(bufs)
    }

    fn seek(&self, pos: SeekFrom) -> Result<off_t, Error> {
        let mut inner_guard = self.inner.lock().unwrap();
        let inner = inner_guard.borrow_mut();
        inner.seek(pos)
    }

    fn metadata(&self) -> Result<Metadata, Error> {
        unimplemented!()
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
        if !self.is_writable {
            return errno!(EINVAL, "File not writable");
        }

        let mut file_guard = self.file.lock().unwrap();
        let file = file_guard.borrow_mut();

        let seek_pos = if !self.is_append {
            SeekFrom::Start(self.pos as u64)
        } else {
            SeekFrom::End(0)
        };
        // TODO: recover from error
        file.seek(seek_pos)
            .map_err(|e| Error::new(Errno::EINVAL, "Failed to seek to a position"))?;

        let write_len = {
            file.write(buf)
                .map_err(|e| Error::new(Errno::EINVAL, "Failed to write"))?
        };

        if !self.is_append {
            self.pos += write_len;
        }
        Ok(write_len)
    }

    pub fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
        if !self.is_readable {
            return errno!(EINVAL, "File not readable");
        }

        let mut file_guard = self.file.lock().unwrap();
        let file = file_guard.borrow_mut();

        let seek_pos = SeekFrom::Start(self.pos as u64);
        file.seek(seek_pos)
            .map_err(|e| Error::new(Errno::EINVAL, "Failed to seek to a position"))?;

        let read_len = {
            file.read(buf)
                .map_err(|e| Error::new(Errno::EINVAL, "Failed to write"))?
        };

        self.pos += read_len;
        Ok(read_len)
    }

    pub fn seek(&mut self, pos: SeekFrom) -> Result<off_t, Error> {
        let mut file_guard = self.file.lock().unwrap();
        let file = file_guard.borrow_mut();

        let pos = match pos {
            SeekFrom::Start(absolute_offset) => pos,
            SeekFrom::End(relative_offset) => pos,
            SeekFrom::Current(relative_offset) => {
                if relative_offset >= 0 {
                    SeekFrom::Start((self.pos + relative_offset as usize) as u64)
                } else {
                    let backward_offset = (-relative_offset) as usize;
                    if self.pos < backward_offset {
                        // underflow
                        return errno!(EINVAL, "Invalid seek position");
                    }
                    SeekFrom::Start((self.pos - backward_offset) as u64)
                }
            }
        };

        self.pos = file
            .seek(pos)
            .map_err(|e| Error::new(Errno::EINVAL, "Failed to seek to a position"))?
            as usize;
        Ok(self.pos as off_t)
    }

    pub fn writev(&mut self, bufs: &[&[u8]]) -> Result<usize, Error> {
        if !self.is_writable {
            return errno!(EINVAL, "File not writable");
        }

        let mut file_guard = self.file.lock().unwrap();
        let file = file_guard.borrow_mut();

        let seek_pos = if !self.is_append {
            SeekFrom::Start(self.pos as u64)
        } else {
            SeekFrom::End(0)
        };
        file.seek(seek_pos)
            .map_err(|e| Error::new(Errno::EINVAL, "Failed to seek to a position"))?;

        let mut total_bytes = 0;
        for buf in bufs {
            match file.write(buf) {
                Ok(this_bytes) => {
                    total_bytes += this_bytes;
                    if this_bytes < buf.len() {
                        break;
                    }
                }
                Err(e) => {
                    match total_bytes {
                        // a complete failure
                        0 => return errno!(EINVAL, "Failed to write"),
                        // a partially failure
                        _ => break,
                    }
                }
            }
        }

        self.pos += total_bytes;
        Ok(total_bytes)
    }

    fn readv(&mut self, bufs: &mut [&mut [u8]]) -> Result<usize, Error> {
        if !self.is_readable {
            return errno!(EINVAL, "File not readable");
        }

        let mut file_guard = self.file.lock().unwrap();
        let file = file_guard.borrow_mut();

        let seek_pos = SeekFrom::Start(self.pos as u64);
        file.seek(seek_pos)
            .map_err(|e| Error::new(Errno::EINVAL, "Failed to seek to a position"))?;

        let mut total_bytes = 0;
        for buf in bufs {
            match file.read(buf) {
                Ok(this_bytes) => {
                    total_bytes += this_bytes;
                    if this_bytes < buf.len() {
                        break;
                    }
                }
                Err(e) => {
                    match total_bytes {
                        // a complete failure
                        0 => return errno!(EINVAL, "Failed to write"),
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
    fn read(&self, buf: &mut [u8]) -> Result<usize, Error> {
        errno!(EBADF, "Stdout does not support read")
    }

    fn write(&self, buf: &[u8]) -> Result<usize, Error> {
        let write_len = {
            self.inner
                .lock()
                .write(buf)
                .map_err(|e| (Errno::EINVAL, "Failed to write"))?
        };
        Ok(write_len)
    }

    fn read_at(&self, _offset: usize, buf: &mut [u8]) -> Result<usize, Error> {
        self.read(buf)
    }

    fn write_at(&self, _offset: usize, buf: &[u8]) -> Result<usize, Error> {
        self.write(buf)
    }

    fn readv(&self, bufs: &mut [&mut [u8]]) -> Result<usize, Error> {
        errno!(EBADF, "Stdout does not support read")
    }

    fn writev(&self, bufs: &[&[u8]]) -> Result<usize, Error> {
        let mut guard = self.inner.lock();
        let mut total_bytes = 0;
        for buf in bufs {
            match guard.write(buf) {
                Ok(this_len) => {
                    total_bytes += this_len;
                    if this_len < buf.len() {
                        break;
                    }
                }
                Err(e) => {
                    match total_bytes {
                        // a complete failure
                        0 => return errno!(EINVAL, "Failed to write"),
                        // a partially failure
                        _ => break,
                    }
                }
            }
        }
        Ok(total_bytes)
    }

    fn seek(&self, seek_pos: SeekFrom) -> Result<off_t, Error> {
        errno!(ESPIPE, "Stdout does not support seek")
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
            type_: FileType::CharDevice,
            mode: 0,
            nlinks: 0,
            uid: 0,
            gid: 0,
            rdev: 0,
        })
    }

    fn set_len(&self, _len: u64) -> Result<(), Error> {
        errno!(EINVAL, "Stdout does not support set_len")
    }

    fn sync_all(&self) -> Result<(), Error> {
        self.sync_data()
    }

    fn sync_data(&self) -> Result<(), Error> {
        self.inner.lock().flush()?;
        Ok(())
    }

    fn read_entry(&self) -> Result<String, Error> {
        errno!(ENOTDIR, "Stdout does not support read_entry")
    }

    fn as_any(&self) -> &Any {
        self
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
            self.inner
                .lock()
                .read(buf)
                .map_err(|e| (Errno::EINVAL, "Failed to read"))?
        };
        Ok(read_len)
    }

    fn write(&self, buf: &[u8]) -> Result<usize, Error> {
        errno!(EBADF, "Stdin does not support write")
    }

    fn read_at(&self, offset: usize, buf: &mut [u8]) -> Result<usize, Error> {
        unimplemented!()
    }

    fn write_at(&self, offset: usize, buf: &[u8]) -> Result<usize, Error> {
        unimplemented!()
    }

    fn readv(&self, bufs: &mut [&mut [u8]]) -> Result<usize, Error> {
        let mut guard = self.inner.lock();
        let mut total_bytes = 0;
        for buf in bufs {
            match guard.read(buf) {
                Ok(this_len) => {
                    total_bytes += this_len;
                    if this_len < buf.len() {
                        break;
                    }
                }
                Err(e) => {
                    match total_bytes {
                        // a complete failure
                        0 => return errno!(EINVAL, "Failed to write"),
                        // a partially failure
                        _ => break,
                    }
                }
            }
        }
        Ok(total_bytes)
    }

    fn writev(&self, bufs: &[&[u8]]) -> Result<usize, Error> {
        errno!(EBADF, "Stdin does not support write")
    }

    fn seek(&self, pos: SeekFrom) -> Result<off_t, Error> {
        errno!(ESPIPE, "Stdin does not support seek")
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
            type_: FileType::CharDevice,
            mode: 0,
            nlinks: 0,
            uid: 0,
            gid: 0,
            rdev: 0,
        })
    }

    fn set_len(&self, _len: u64) -> Result<(), Error> {
        errno!(EINVAL, "Stdin does not support set_len")
    }

    fn sync_all(&self) -> Result<(), Error> {
        self.sync_data()
    }

    fn sync_data(&self) -> Result<(), Error> {
        Ok(())
    }

    fn read_entry(&self) -> Result<String, Error> {
        errno!(ENOTDIR, "Stdin does not support read_entry")
    }

    fn as_any(&self) -> &Any {
        self
    }
}

impl Debug for StdinFile {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "StdinFile")
    }
}

unsafe impl Send for StdinFile {}
unsafe impl Sync for StdinFile {}
