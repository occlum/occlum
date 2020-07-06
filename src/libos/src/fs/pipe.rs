use super::*;
use net::{IoEvent, PollEventFlags};
use util::ring_buf::*;

// TODO: Add F_SETPIPE_SZ in fcntl to dynamically change the size of pipe
// to improve memory efficiency. This value is got from /proc/sys/fs/pipe-max-size on linux.
pub const PIPE_BUF_SIZE: usize = 1024 * 1024;

pub fn pipe(flags: StatusFlags) -> Result<(PipeReader, PipeWriter)> {
    let (buffer_reader, buffer_writer) =
        ring_buffer(PIPE_BUF_SIZE).map_err(|e| errno!(ENFILE, "No memory for new pipes"))?;
    // Only O_NONBLOCK and O_DIRECT can be applied during pipe creation
    let valid_flags = flags & (StatusFlags::O_NONBLOCK | StatusFlags::O_DIRECT);

    if flags.contains(StatusFlags::O_NONBLOCK) {
        buffer_reader.set_non_blocking();
        buffer_writer.set_non_blocking();
    }

    Ok((
        PipeReader {
            inner: SgxMutex::new(buffer_reader),
            status_flags: RwLock::new(valid_flags),
        },
        PipeWriter {
            inner: SgxMutex::new(buffer_writer),
            status_flags: RwLock::new(valid_flags),
        },
    ))
}

pub struct PipeReader {
    inner: SgxMutex<RingBufReader>,
    status_flags: RwLock<StatusFlags>,
}

impl File for PipeReader {
    fn read(&self, buf: &mut [u8]) -> Result<usize> {
        let mut ringbuf = self.inner.lock().unwrap();
        ringbuf.read_from_buffer(buf)
    }

    fn readv(&self, bufs: &mut [&mut [u8]]) -> Result<usize> {
        let mut ringbuf = self.inner.lock().unwrap();
        ringbuf.read_from_vector(bufs)
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

        if new_status_flags.contains(StatusFlags::O_NONBLOCK) {
            self.inner.lock().unwrap().set_non_blocking();
        } else {
            self.inner.lock().unwrap().set_blocking();
        }
        Ok(())
    }

    fn poll(&self) -> Result<PollEventFlags> {
        let ringbuf_reader = self.inner.lock().unwrap();
        let mut events = PollEventFlags::empty();
        if ringbuf_reader.can_read() {
            events |= PollEventFlags::POLLIN | PollEventFlags::POLLRDNORM;
        }

        if ringbuf_reader.is_peer_closed() {
            events |= PollEventFlags::POLLHUP;
        }

        Ok(events)
    }

    fn enqueue_event(&self, event: IoEvent) -> Result<()> {
        let ringbuf_reader = self.inner.lock().unwrap();
        ringbuf_reader.enqueue_event(event)
    }

    fn dequeue_event(&self) -> Result<()> {
        let ringbuf_reader = self.inner.lock().unwrap();
        ringbuf_reader.dequeue_event()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

unsafe impl Send for PipeReader {}
unsafe impl Sync for PipeReader {}

pub struct PipeWriter {
    inner: SgxMutex<RingBufWriter>,
    status_flags: RwLock<StatusFlags>,
}

impl File for PipeWriter {
    fn write(&self, buf: &[u8]) -> Result<usize> {
        let mut ringbuf = self.inner.lock().unwrap();
        ringbuf.write_to_buffer(buf)
    }

    fn writev(&self, bufs: &[&[u8]]) -> Result<usize> {
        let mut ringbuf = self.inner.lock().unwrap();
        ringbuf.write_to_vector(bufs)
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

        if new_status_flags.contains(StatusFlags::O_NONBLOCK) {
            self.inner.lock().unwrap().set_non_blocking();
        } else {
            self.inner.lock().unwrap().set_blocking();
        }
        Ok(())
    }

    fn poll(&self) -> Result<PollEventFlags> {
        let ringbuf_writer = self.inner.lock().unwrap();
        let mut events = PollEventFlags::empty();
        if ringbuf_writer.can_write() {
            events |= PollEventFlags::POLLOUT | PollEventFlags::POLLWRNORM;
        }
        if ringbuf_writer.is_peer_closed() {
            events |= PollEventFlags::POLLERR;
        }

        Ok(events)
    }

    fn enqueue_event(&self, event: IoEvent) -> Result<()> {
        let ringbuf_writer = self.inner.lock().unwrap();
        ringbuf_writer.enqueue_event(event)
    }

    fn dequeue_event(&self) -> Result<()> {
        let ringbuf_writer = self.inner.lock().unwrap();
        ringbuf_writer.dequeue_event()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl fmt::Debug for PipeReader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PipeReader")
            .field("status_flags", &self.status_flags)
            .finish()
    }
}

impl fmt::Debug for PipeWriter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PipeWriter")
            .field("status_flags", &self.status_flags)
            .finish()
    }
}

unsafe impl Send for PipeWriter {}
unsafe impl Sync for PipeWriter {}

pub fn do_pipe2(flags: u32) -> Result<[FileDesc; 2]> {
    let creation_flags = CreationFlags::from_bits_truncate(flags);
    let status_flags = StatusFlags::from_bits_truncate(flags);
    debug!("pipe2: flags: {:?} {:?}", creation_flags, status_flags);

    let (pipe_reader, pipe_writer) = pipe(status_flags)?;
    let close_on_spawn = creation_flags.must_close_on_spawn();

    let current = current!();
    let reader_fd = current.add_file(Arc::new(Box::new(pipe_reader)), close_on_spawn);
    let writer_fd = current.add_file(Arc::new(Box::new(pipe_writer)), close_on_spawn);
    trace!("pipe2: reader_fd: {}, writer_fd: {}", reader_fd, writer_fd);
    Ok([reader_fd, writer_fd])
}

pub trait PipeType {
    fn as_pipe_reader(&self) -> Result<&PipeReader>;
    fn as_pipe_writer(&self) -> Result<&PipeWriter>;
}
impl PipeType for FileRef {
    fn as_pipe_reader(&self) -> Result<&PipeReader> {
        self.as_any()
            .downcast_ref::<PipeReader>()
            .ok_or_else(|| errno!(EBADF, "not a pipe reader"))
    }
    fn as_pipe_writer(&self) -> Result<&PipeWriter> {
        self.as_any()
            .downcast_ref::<PipeWriter>()
            .ok_or_else(|| errno!(EBADF, "not a pipe writer"))
    }
}
