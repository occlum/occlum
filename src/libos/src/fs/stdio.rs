use super::*;
use core::cell::RefCell;
use core::cmp;
use std::io::{BufReader, LineWriter};
use std::sync::SgxMutex;

macro_rules! try_libc_stdio {
    ($ret: expr) => {{
        let ret = unsafe { $ret };
        if ret < 0 {
            let errno_c = unsafe { libc::errno() };
            Err(errno!(Errno::from(errno_c as u32)))
        } else {
            Ok(ret)
        }
    }};
}

// Struct for the occlum_stdio_fds
#[repr(C)]
pub struct HostStdioFds {
    pub stdin_fd: i32,
    pub stdout_fd: i32,
    pub stderr_fd: i32,
}

impl HostStdioFds {
    pub fn from_user(ptr: *const HostStdioFds) -> Result<Self> {
        if ptr.is_null() {
            return Ok(Self {
                stdin_fd: libc::STDIN_FILENO,
                stdout_fd: libc::STDOUT_FILENO,
                stderr_fd: libc::STDERR_FILENO,
            });
        }
        let host_stdio_fds_c = unsafe { &*ptr };
        if host_stdio_fds_c.stdin_fd < 0
            || host_stdio_fds_c.stdout_fd < 0
            || host_stdio_fds_c.stderr_fd < 0
        {
            return_errno!(EBADF, "invalid file descriptor");
        }
        Ok(Self {
            stdin_fd: host_stdio_fds_c.stdin_fd,
            stdout_fd: host_stdio_fds_c.stdout_fd,
            stderr_fd: host_stdio_fds_c.stderr_fd,
        })
    }
}

struct StdoutRaw {
    host_fd: i32,
}

impl StdoutRaw {
    pub fn new(host_fd: FileDesc) -> Self {
        Self {
            host_fd: host_fd as i32,
        }
    }
}

impl std::io::Write for StdoutRaw {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let writting_len = cmp::min(buf.len(), size_t::max_value() as usize);
        let (buf_ptr, _) = buf.as_ptr_and_len();
        let ret = try_libc_stdio!(libc::ocall::write(
            self.host_fd,
            buf_ptr as *const c_void,
            writting_len,
        ))
        .unwrap_or_else(|err| {
            warn!("tolerate the write error: {:?}", err.errno());
            writting_len as isize
        });
        // sanity check
        assert!(ret <= writting_len as isize);
        Ok(ret as usize)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub struct StdoutFile {
    inner: SgxMutex<LineWriter<StdoutRaw>>,
    host_fd: FileDesc,
}

impl StdoutFile {
    pub fn new(host_fd: FileDesc) -> Self {
        StdoutFile {
            inner: SgxMutex::new(LineWriter::new(StdoutRaw::new(host_fd))),
            host_fd,
        }
    }

    fn host_fd(&self) -> FileDesc {
        self.host_fd
    }
}

impl File for StdoutFile {
    fn write(&self, buf: &[u8]) -> Result<usize> {
        let write_len = {
            self.inner
                .lock()
                .unwrap()
                .write(buf)
                .map_err(|e| errno!(e))?
        };
        Ok(write_len)
    }

    fn writev(&self, bufs: &[&[u8]]) -> Result<usize> {
        let mut guard = self.inner.lock().unwrap();
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

    fn poll(&self, mask: Events, _poller: Option<&Poller>) -> Events {
        Events::OUT
    }

    fn ioctl(&self, cmd: &mut dyn IoctlCmd) -> Result<()> {
        stdio_ioctl(cmd, self.host_fd())
    }

    fn status_flags(&self) -> StatusFlags {
        let ret = try_libc_stdio!(libc::ocall::fcntl_arg0(
            self.host_fd() as i32,
            libc::F_GETFL
        ))
        .unwrap_or_else(|err| {
            warn!("failed to getfl for stdout, error: {:?}", err.errno());
            StatusFlags::empty().bits() as i32
        });

        StatusFlags::from_bits_truncate(ret as u32)
    }

    fn set_status_flags(&self, new_status_flags: StatusFlags) -> Result<()> {
        let raw_status_flags = (new_status_flags & STATUS_FLAGS_MASK).bits();
        try_libc!(libc::ocall::fcntl_arg1(
            self.host_fd() as i32,
            libc::F_SETFL,
            raw_status_flags as c_int
        ));
        Ok(())
    }

    fn seek(&self, pos: SeekFrom) -> Result<usize> {
        let (off, whence) = match pos {
            SeekFrom::Start(off) => (off as off_t, 0 /* SEEK_SET */),
            SeekFrom::Current(off) => (off as off_t, 1 /* SEEK_CUR */),
            SeekFrom::End(off) => (off as off_t, 2 /* SEEK_END */),
        };
        let offset = try_libc!(libc::ocall::lseek(self.host_fd() as i32, off, whence));
        Ok(offset as usize)
    }
}

impl Debug for StdoutFile {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "StdoutFile with host_fd: {}", self.host_fd)
    }
}

unsafe impl Send for StdoutFile {}
unsafe impl Sync for StdoutFile {}

struct StdinRaw {
    host_fd: i32,
}

impl StdinRaw {
    pub fn new(host_fd: FileDesc) -> Self {
        Self {
            host_fd: host_fd as i32,
        }
    }
}

impl std::io::Read for StdinRaw {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let reading_len = cmp::min(buf.len(), size_t::max_value() as usize);
        let (buf_ptr, _) = buf.as_mut().as_mut_ptr_and_len();
        let ret = try_libc_stdio!(libc::ocall::read(
            self.host_fd,
            buf_ptr as *mut c_void,
            reading_len,
        ))
        .unwrap_or_else(|err| {
            warn!("tolerate the read error: {:?}", err.errno());
            0
        });
        // sanity check
        assert!(ret <= reading_len as isize);
        Ok(ret as usize)
    }
}

pub struct StdinFile {
    inner: SgxMutex<BufReader<StdinRaw>>,
    host_fd: FileDesc,
}

impl StdinFile {
    pub fn new(host_fd: FileDesc) -> Self {
        StdinFile {
            inner: SgxMutex::new(BufReader::new(StdinRaw::new(host_fd))),
            host_fd,
        }
    }

    fn host_fd(&self) -> FileDesc {
        self.host_fd
    }
}

impl File for StdinFile {
    fn read(&self, buf: &mut [u8]) -> Result<usize> {
        let read_len = {
            self.inner
                .lock()
                .unwrap()
                .read(buf)
                .map_err(|e| errno!(e))?
        };
        Ok(read_len)
    }

    fn readv(&self, bufs: &mut [&mut [u8]]) -> Result<usize> {
        let mut guard = self.inner.lock().unwrap();
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

    fn poll(&self, mask: Events, _poller: Option<&Poller>) -> Events {
        Events::IN
    }

    fn ioctl(&self, cmd: &mut dyn IoctlCmd) -> Result<()> {
        stdio_ioctl(cmd, self.host_fd())
    }

    fn status_flags(&self) -> StatusFlags {
        let ret = try_libc_stdio!(libc::ocall::fcntl_arg0(
            self.host_fd() as i32,
            libc::F_GETFL
        ))
        .unwrap_or_else(|err| {
            warn!("failed to getfl for stdin, error: {:?}", err.errno());
            StatusFlags::empty().bits() as i32
        });

        StatusFlags::from_bits_truncate(ret as u32)
    }

    fn set_status_flags(&self, new_status_flags: StatusFlags) -> Result<()> {
        let raw_status_flags = (new_status_flags & STATUS_FLAGS_MASK).bits();
        try_libc!(libc::ocall::fcntl_arg1(
            self.host_fd() as i32,
            libc::F_SETFL,
            raw_status_flags as c_int
        ));
        Ok(())
    }

    fn seek(&self, pos: SeekFrom) -> Result<usize> {
        let (off, whence) = match pos {
            SeekFrom::Start(off) => (off as off_t, 0 /* SEEK_SET */),
            SeekFrom::Current(off) => (off as off_t, 1 /* SEEK_CUR */),
            SeekFrom::End(off) => (off as off_t, 2 /* SEEK_END */),
        };
        let offset = try_libc!(libc::ocall::lseek(self.host_fd() as i32, off, whence));
        Ok(offset as usize)
    }
}

impl Debug for StdinFile {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "StdinFile with host_fd: {}", self.host_fd)
    }
}

unsafe impl Send for StdinFile {}
unsafe impl Sync for StdinFile {}

fn stdio_ioctl(cmd: &mut dyn IoctlCmd, host_fd: FileDesc) -> Result<()> {
    debug!("stdio ioctl: cmd: {:?}", cmd);
    async_io::match_ioctl_cmd_auto_error!(cmd, {
        cmd : TcGets => {
            cmd.execute(host_fd)?
        },
        cmd : TcSets => {
            cmd.execute(host_fd)?
        },
        cmd : SetWinSize => {
            cmd.execute(host_fd)?
        },
        cmd : GetWinSize => {
            cmd.execute(host_fd)?
        },
    });
    Ok(())
}
