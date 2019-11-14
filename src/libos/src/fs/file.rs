use super::*;
use std;
use std::borrow::BorrowMut;
use std::fmt;
use std::io::SeekFrom;

macro_rules! return_op_unsupported_error {
    ($op_name: expr, $errno: expr) => {{
        let errno = $errno;
        // FIXME: use the safe core::any::type_name when we upgrade to Rust 1.38 or above
        let type_name = unsafe { core::intrinsics::type_name::<Self>() };
        let op_name = $op_name;
        let error = FileOpNotSupportedError::new(errno, type_name, op_name);
        return_errno!(error)
    }};
    ($op_name: expr) => {{
        return_op_unsupported_error!($op_name, ENOSYS)
    }};
}

pub trait File: Debug + Sync + Send + Any {
    fn read(&self, buf: &mut [u8]) -> Result<usize> {
        return_op_unsupported_error!("read")
    }

    fn write(&self, buf: &[u8]) -> Result<usize> {
        return_op_unsupported_error!("write")
    }

    fn read_at(&self, offset: usize, buf: &mut [u8]) -> Result<usize> {
        return_op_unsupported_error!("read_at")
    }

    fn write_at(&self, offset: usize, buf: &[u8]) -> Result<usize> {
        return_op_unsupported_error!("write_at")
    }

    fn readv(&self, bufs: &mut [&mut [u8]]) -> Result<usize> {
        return_op_unsupported_error!("readv")
    }

    fn writev(&self, bufs: &[&[u8]]) -> Result<usize> {
        return_op_unsupported_error!("writev")
    }

    fn seek(&self, pos: SeekFrom) -> Result<off_t> {
        return_op_unsupported_error!("seek")
    }

    fn metadata(&self) -> Result<Metadata> {
        return_op_unsupported_error!("metadata")
    }

    fn set_len(&self, len: u64) -> Result<()> {
        return_op_unsupported_error!("set_len")
    }

    fn read_entry(&self) -> Result<String> {
        return_op_unsupported_error!("read_entry", ENOTDIR)
    }

    fn sync_all(&self) -> Result<()> {
        Ok(())
    }

    fn sync_data(&self) -> Result<()> {
        Ok(())
    }

    fn ioctl(&self, cmd: &mut IoctlCmd) -> Result<()> {
        return_op_unsupported_error!("ioctl")
    }

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
    ) -> Result<SgxFile> {
        if !is_readable && !is_writable {
            return_errno!(EINVAL, "Invalid permissions");
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
    fn read(&self, buf: &mut [u8]) -> Result<usize> {
        let mut inner_guard = self.inner.lock().unwrap();
        let inner = inner_guard.borrow_mut();
        inner.read(buf)
    }

    fn write(&self, buf: &[u8]) -> Result<usize> {
        let mut inner_guard = self.inner.lock().unwrap();
        let inner = inner_guard.borrow_mut();
        inner.write(buf)
    }

    fn read_at(&self, offset: usize, buf: &mut [u8]) -> Result<usize> {
        let mut inner_guard = self.inner.lock().unwrap();
        let inner = inner_guard.borrow_mut();
        inner.seek(SeekFrom::Start(offset as u64))?;
        inner.read(buf)
    }

    fn write_at(&self, offset: usize, buf: &[u8]) -> Result<usize> {
        let mut inner_guard = self.inner.lock().unwrap();
        let inner = inner_guard.borrow_mut();
        inner.seek(SeekFrom::Start(offset as u64))?;
        inner.write(buf)
    }

    fn readv(&self, bufs: &mut [&mut [u8]]) -> Result<usize> {
        let mut inner_guard = self.inner.lock().unwrap();
        let inner = inner_guard.borrow_mut();
        inner.readv(bufs)
    }

    fn writev(&self, bufs: &[&[u8]]) -> Result<usize> {
        let mut inner_guard = self.inner.lock().unwrap();
        let inner = inner_guard.borrow_mut();
        inner.writev(bufs)
    }

    fn seek(&self, pos: SeekFrom) -> Result<off_t> {
        let mut inner_guard = self.inner.lock().unwrap();
        let inner = inner_guard.borrow_mut();
        inner.seek(pos)
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
    pub fn write(&mut self, buf: &[u8]) -> Result<usize> {
        if !self.is_writable {
            return_errno!(EINVAL, "File not writable");
        }

        let mut file_guard = self.file.lock().unwrap();
        let file = file_guard.borrow_mut();

        let seek_pos = if !self.is_append {
            SeekFrom::Start(self.pos as u64)
        } else {
            SeekFrom::End(0)
        };
        // TODO: recover from error
        file.seek(seek_pos).map_err(|e| errno!(e))?;

        let write_len = { file.write(buf).map_err(|e| errno!(e))? };

        if !self.is_append {
            self.pos += write_len;
        }
        Ok(write_len)
    }

    pub fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        if !self.is_readable {
            return_errno!(EINVAL, "File not readable");
        }

        let mut file_guard = self.file.lock().unwrap();
        let file = file_guard.borrow_mut();

        let seek_pos = SeekFrom::Start(self.pos as u64);
        file.seek(seek_pos).map_err(|e| errno!(e))?;

        let read_len = { file.read(buf).map_err(|e| errno!(e))? };

        self.pos += read_len;
        Ok(read_len)
    }

    pub fn seek(&mut self, pos: SeekFrom) -> Result<off_t> {
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
                        return_errno!(EINVAL, "Invalid seek position");
                    }
                    SeekFrom::Start((self.pos - backward_offset) as u64)
                }
            }
        };

        self.pos = file.seek(pos).map_err(|e| errno!(e))? as usize;
        Ok(self.pos as off_t)
    }

    pub fn writev(&mut self, bufs: &[&[u8]]) -> Result<usize> {
        if !self.is_writable {
            return_errno!(EINVAL, "File not writable");
        }

        let mut file_guard = self.file.lock().unwrap();
        let file = file_guard.borrow_mut();

        let seek_pos = if !self.is_append {
            SeekFrom::Start(self.pos as u64)
        } else {
            SeekFrom::End(0)
        };
        file.seek(seek_pos).map_err(|e| errno!(e))?;

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
                        0 => return_errno!(EINVAL, "Failed to write"),
                        // a partially failure
                        _ => break,
                    }
                }
            }
        }

        self.pos += total_bytes;
        Ok(total_bytes)
    }

    fn readv(&mut self, bufs: &mut [&mut [u8]]) -> Result<usize> {
        if !self.is_readable {
            return_errno!(EINVAL, "File not readable");
        }

        let mut file_guard = self.file.lock().unwrap();
        let file = file_guard.borrow_mut();

        let seek_pos = SeekFrom::Start(self.pos as u64);
        file.seek(seek_pos).map_err(|e| errno!(e))?;

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
                        0 => return_errno!(EINVAL, "Failed to write"),
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
    fn write(&self, buf: &[u8]) -> Result<usize> {
        let write_len = { self.inner.lock().write(buf).map_err(|e| errno!(e))? };
        Ok(write_len)
    }

    fn write_at(&self, _offset: usize, buf: &[u8]) -> Result<usize> {
        self.write(buf)
    }

    fn writev(&self, bufs: &[&[u8]]) -> Result<usize> {
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
                        0 => return_errno!(EINVAL, "Failed to write"),
                        // a partially failure
                        _ => break,
                    }
                }
            }
        }
        Ok(total_bytes)
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
            type_: FileType::CharDevice,
            mode: 0,
            nlinks: 0,
            uid: 0,
            gid: 0,
            rdev: 0,
        })
    }

    fn sync_all(&self) -> Result<()> {
        self.sync_data()
    }

    fn sync_data(&self) -> Result<()> {
        self.inner.lock().flush()?;
        Ok(())
    }

    fn ioctl(&self, cmd: &mut IoctlCmd) -> Result<()> {
        let can_delegate_to_host = match cmd {
            IoctlCmd::TIOCGWINSZ(_) => true,
            IoctlCmd::TIOCSWINSZ(_) => true,
            _ => false,
        };
        if !can_delegate_to_host {
            return_errno!(EINVAL, "unknown ioctl cmd for stdout");
        }

        let cmd_bits = cmd.cmd_num() as c_int;
        let cmd_arg_ptr = cmd.arg_ptr() as *const c_int;
        let host_stdout_fd = {
            use std::os::unix::io::AsRawFd;
            self.inner.as_raw_fd() as i32
        };
        try_libc!(libc::ocall::ioctl_arg1(
            host_stdout_fd,
            cmd_bits,
            cmd_arg_ptr
        ));
        cmd.validate_arg_val()?;

        Ok(())
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
    fn read(&self, buf: &mut [u8]) -> Result<usize> {
        let read_len = { self.inner.lock().read(buf).map_err(|e| errno!(e))? };
        Ok(read_len)
    }

    fn readv(&self, bufs: &mut [&mut [u8]]) -> Result<usize> {
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
                        0 => return_errno!(EINVAL, "Failed to write"),
                        // a partially failure
                        _ => break,
                    }
                }
            }
        }
        Ok(total_bytes)
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
            type_: FileType::CharDevice,
            mode: 0,
            nlinks: 0,
            uid: 0,
            gid: 0,
            rdev: 0,
        })
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

#[derive(Copy, Clone, Debug)]
struct FileOpNotSupportedError {
    errno: Errno,
    type_name: &'static str,
    op_name: &'static str,
}

impl FileOpNotSupportedError {
    pub fn new(
        errno: Errno,
        type_name: &'static str,
        op_name: &'static str,
    ) -> FileOpNotSupportedError {
        FileOpNotSupportedError {
            errno,
            type_name,
            op_name,
        }
    }
}

impl fmt::Display for FileOpNotSupportedError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} does not support {}", self.type_name, self.op_name)
    }
}

impl ToErrno for FileOpNotSupportedError {
    fn errno(&self) -> Errno {
        self.errno
    }
}
