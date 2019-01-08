use super::*;
use util::ring_buf::{*};

// TODO: Use Waiter and WaitQueue infrastructure to sleep when blocking

pub const PIPE_BUF_SIZE : usize = 2 * 1024 * 1024;

#[derive(Debug)]
pub struct Pipe {
    pub reader: PipeReader,
    pub writer: PipeWriter,
}

impl Pipe {
    pub fn new() -> Result<Pipe, Error> {
        let mut ring_buf = RingBuf::new(PIPE_BUF_SIZE);
        Ok(Pipe {
            reader: PipeReader {
                inner: SgxMutex::new(ring_buf.reader),
            },
            writer: PipeWriter {
                inner: SgxMutex::new(ring_buf.writer),
            }
        })
    }
}


#[derive(Debug)]
pub struct PipeReader {
    inner: SgxMutex<RingBufReader>,
}

impl File for PipeReader {
    fn read(&self, buf: &mut [u8]) -> Result<usize, Error> {
        let ringbuf = self.inner.lock().unwrap();
        ringbuf.read(buf)
    }

    fn readv<'a, 'b>(&self, bufs: &'a mut [&'b mut [u8]]) -> Result<usize, Error> {
        let mut ringbuf = self.inner.lock().unwrap();
        let mut total_bytes = 0;
        for buf in bufs {
            match ringbuf.read(buf) {
                Ok(this_len) => {
                    total_bytes += this_len;
                    if this_len < buf.len() { break; }
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

    fn write(&self, buf: &[u8]) -> Result<usize, Error> {
        Err(Error::new(Errno::EBADF, "PipeReader does not support write"))
    }

    fn writev<'a, 'b>(&self, bufs: &'a [&'b [u8]]) -> Result<usize, Error> {
        Err(Error::new(Errno::EBADF, "PipeReader does not support write"))
    }

    fn seek(&self, pos: SeekFrom) -> Result<off_t, Error> {
        Err(Error::new(Errno::ESPIPE, "Pipe does not support seek"))
    }
}

unsafe impl Send for PipeReader {}
unsafe impl Sync for PipeReader {}


#[derive(Debug)]
pub struct PipeWriter {
    inner: SgxMutex<RingBufWriter>,
}

impl File for PipeWriter {
    fn write(&self, buf: &[u8]) -> Result<usize, Error> {
        let ringbuf = self.inner.lock().unwrap();
        ringbuf.write(buf)
    }

    fn writev<'a, 'b>(&self, bufs: &'a [&'b [u8]]) -> Result<usize, Error> {
        let ringbuf = self.inner.lock().unwrap();
        let mut total_bytes = 0;
        for buf in bufs {
            match ringbuf.write(buf) {
                Ok(this_len) => {
                    total_bytes += this_len;
                    if this_len < buf.len() { break; }
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

    fn read(&self, buf: &mut [u8]) -> Result<usize, Error> {
        Err(Error::new(Errno::EBADF, "PipeWriter does not support read"))
    }

    fn readv<'a, 'b>(&self, bufs: &'a mut [&'b mut [u8]]) -> Result<usize, Error> {
        Err(Error::new(Errno::EBADF, "PipeWriter does not support read"))
    }

    fn seek(&self, seek_pos: SeekFrom) -> Result<off_t, Error> {
        Err(Error::new(Errno::ESPIPE, "Pipe does not support seek"))
    }
}

unsafe impl Send for PipeWriter {}
unsafe impl Sync for PipeWriter {}
