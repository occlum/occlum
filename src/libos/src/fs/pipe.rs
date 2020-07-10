use super::*;
use util::ring_buf::*;

// TODO: Use Waiter and WaitQueue infrastructure to sleep when blocking
// TODO: Add F_SETPIPE_SZ in fcntl to dynamically change the size of pipe
// to improve memory efficiency. This value is got from /proc/sys/fs/pipe-max-size on linux.
pub const PIPE_BUF_SIZE: usize = 1024 * 1024;

#[derive(Debug)]
pub struct Pipe {
    pub reader: PipeReader,
    pub writer: PipeWriter,
}

impl Pipe {
    pub fn new(flags: StatusFlags) -> Result<Pipe> {
        let mut ring_buf =
            RingBuf::new(PIPE_BUF_SIZE).map_err(|e| errno!(ENFILE, "No memory for new pipes"))?;
        // Only O_NONBLOCK and O_DIRECT can be applied during pipe creation
        let valid_flags = flags & (StatusFlags::O_NONBLOCK | StatusFlags::O_DIRECT);
        Ok(Pipe {
            reader: PipeReader {
                inner: SgxMutex::new(ring_buf.reader),
                status_flags: RwLock::new(valid_flags),
            },
            writer: PipeWriter {
                inner: SgxMutex::new(ring_buf.writer),
                status_flags: RwLock::new(valid_flags),
            },
        })
    }
}

#[derive(Debug)]
pub struct PipeReader {
    inner: SgxMutex<RingBufReader>,
    status_flags: RwLock<StatusFlags>,
}

impl File for PipeReader {
    fn read(&self, buf: &mut [u8]) -> Result<usize> {
        let ringbuf = self.inner.lock().unwrap();
        ringbuf.read(buf)
    }

    fn readv(&self, bufs: &mut [&mut [u8]]) -> Result<usize> {
        let mut ringbuf = self.inner.lock().unwrap();
        let mut total_bytes = 0;
        for buf in bufs {
            match ringbuf.read(buf) {
                Ok(this_len) => {
                    total_bytes += this_len;
                    if this_len < buf.len() {
                        break;
                    }
                }
                Err(e) => {
                    match total_bytes {
                        // a complete failure
                        0 => return Err(e),
                        // a partially failure
                        _ => break,
                    }
                }
            }
        }
        Ok(total_bytes)
    }

    fn get_access_mode(&self) -> Result<AccessMode> {
        Ok(AccessMode::O_RDONLY)
    }

    fn get_status_flags(&self) -> Result<StatusFlags> {
        let status_flags = self.status_flags.read().unwrap();
        Ok(status_flags.clone())
    }

    fn set_status_flags(&self, new_status_flags: StatusFlags) -> Result<()> {
        let mut status_flags = self.status_flags.write().unwrap();
        // Only O_NONBLOCK, O_ASYNC and O_DIRECT can be set
        *status_flags = new_status_flags
            & (StatusFlags::O_NONBLOCK | StatusFlags::O_ASYNC | StatusFlags::O_DIRECT);
        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

unsafe impl Send for PipeReader {}
unsafe impl Sync for PipeReader {}

#[derive(Debug)]
pub struct PipeWriter {
    inner: SgxMutex<RingBufWriter>,
    status_flags: RwLock<StatusFlags>,
}

impl File for PipeWriter {
    fn write(&self, buf: &[u8]) -> Result<usize> {
        let ringbuf = self.inner.lock().unwrap();
        ringbuf.write(buf)
    }

    fn writev(&self, bufs: &[&[u8]]) -> Result<usize> {
        let ringbuf = self.inner.lock().unwrap();
        let mut total_bytes = 0;
        for buf in bufs {
            match ringbuf.write(buf) {
                Ok(this_len) => {
                    total_bytes += this_len;
                    if this_len < buf.len() {
                        break;
                    }
                }
                Err(e) => {
                    match total_bytes {
                        // a complete failure
                        0 => return Err(e),
                        // a partially failure
                        _ => break,
                    }
                }
            }
        }
        Ok(total_bytes)
    }

    fn seek(&self, pos: SeekFrom) -> Result<off_t> {
        return_errno!(ESPIPE, "Pipe does not support seek")
    }

    fn get_access_mode(&self) -> Result<AccessMode> {
        Ok(AccessMode::O_WRONLY)
    }

    fn get_status_flags(&self) -> Result<StatusFlags> {
        let status_flags = self.status_flags.read().unwrap();
        Ok(status_flags.clone())
    }

    fn set_status_flags(&self, new_status_flags: StatusFlags) -> Result<()> {
        let mut status_flags = self.status_flags.write().unwrap();
        // Only O_NONBLOCK, O_ASYNC and O_DIRECT can be set
        *status_flags = new_status_flags
            & (StatusFlags::O_NONBLOCK | StatusFlags::O_ASYNC | StatusFlags::O_DIRECT);
        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

unsafe impl Send for PipeWriter {}
unsafe impl Sync for PipeWriter {}

pub fn do_pipe2(flags: u32) -> Result<[FileDesc; 2]> {
    let creation_flags = CreationFlags::from_bits_truncate(flags);
    let status_flags = StatusFlags::from_bits_truncate(flags);
    debug!("pipe2: flags: {:?} {:?}", creation_flags, status_flags);

    let current = current!();
    let pipe = Pipe::new(status_flags)?;
    let close_on_spawn = creation_flags.must_close_on_spawn();
    let reader_fd = current.add_file(Arc::new(Box::new(pipe.reader)), close_on_spawn);
    let writer_fd = current.add_file(Arc::new(Box::new(pipe.writer)), close_on_spawn);
    trace!("pipe2: reader_fd: {}, writer_fd: {}", reader_fd, writer_fd);
    Ok([reader_fd, writer_fd])
}
