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

    fn write_at(&self, _offset: usize, buf: &[u8]) -> Result<usize> {
        self.write(buf)
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
        self.inner.lock().unwrap().flush()?;
        Ok(())
    }

    fn ioctl(&self, cmd: &mut IoctlCmd) -> Result<i32> {
        let host_stdout_fd = self.host_fd() as i32;
        let cmd_bits = cmd.cmd_num() as c_int;

        // Handle special case for TCGETS/TCSETS which use different structures
        // in linux kernel and libc
        match cmd {
            IoctlCmd::TCGETS(kernel_termios) => {
                return kernel_termios.execute_tcgets(host_stdout_fd, cmd_bits);
            }
            IoctlCmd::TCSETS(kernel_termios) => {
                return kernel_termios.execute_tcsets(host_stdout_fd, cmd_bits);
            }
            _ => {}
        };

        let can_delegate_to_host = match cmd {
            IoctlCmd::TIOCGWINSZ(_) => true,
            IoctlCmd::TIOCSWINSZ(_) => true,
            _ => false,
        };
        if !can_delegate_to_host {
            return_errno!(EINVAL, "unknown ioctl cmd for stdout");
        }

        let cmd_arg_ptr = cmd.arg_ptr() as *mut c_void;
        let cmd_arg_len = cmd.arg_len();
        let ret = try_libc!({
            let mut retval: i32 = 0;
            let status = occlum_ocall_ioctl(
                &mut retval as *mut i32,
                host_stdout_fd,
                cmd_bits,
                cmd_arg_ptr,
                cmd_arg_len,
            );
            assert!(status == sgx_status_t::SGX_SUCCESS);
            retval
        });
        cmd.validate_arg_and_ret_vals(ret)?;

        Ok(ret)
    }

    fn as_any(&self) -> &dyn Any {
        self
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

    fn ioctl(&self, cmd: &mut IoctlCmd) -> Result<i32> {
        let host_stdin_fd = self.host_fd() as i32;
        let cmd_bits = cmd.cmd_num() as c_int;

        // Handle special case for TCGETS/TCSETS which use different structures
        // in linux kernel and libc
        match cmd {
            IoctlCmd::TCGETS(kernel_termios) => {
                return kernel_termios.execute_tcgets(host_stdin_fd, cmd_bits);
            }
            IoctlCmd::TCSETS(kernel_termios) => {
                return kernel_termios.execute_tcsets(host_stdin_fd, cmd_bits);
            }
            _ => {}
        };

        let can_delegate_to_host = match cmd {
            IoctlCmd::TIOCGWINSZ(_) => true,
            IoctlCmd::TIOCSWINSZ(_) => true,
            _ => false,
        };
        if !can_delegate_to_host {
            return_errno!(EINVAL, "unknown ioctl cmd for stdin");
        }

        let cmd_arg_ptr = cmd.arg_ptr() as *mut c_void;
        let cmd_arg_len = cmd.arg_len();
        let ret = try_libc!({
            let mut retval: i32 = 0;
            let status = occlum_ocall_ioctl(
                &mut retval as *mut i32,
                host_stdin_fd,
                cmd_bits,
                cmd_arg_ptr,
                cmd_arg_len,
            );
            assert!(status == sgx_status_t::SGX_SUCCESS);
            retval
        });
        cmd.validate_arg_and_ret_vals(ret)?;

        Ok(ret)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Debug for StdinFile {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "StdinFile with host_fd: {}", self.host_fd)
    }
}

unsafe impl Send for StdinFile {}
unsafe impl Sync for StdinFile {}
