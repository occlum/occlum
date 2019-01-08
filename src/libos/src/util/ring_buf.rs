use super::{*};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, AtomicBool, Ordering};
use std::cmp::{min, max};
use std::{ptr};

#[derive(Debug)]
pub struct RingBuf {
    pub reader: RingBufReader,
    pub writer: RingBufWriter,
}

impl RingBuf {
    pub fn new(capacity: usize) -> RingBuf {
        let inner = Arc::new(RingBufInner::new(capacity));
        let reader = RingBufReader { inner: inner.clone() };
        let writer = RingBufWriter { inner: inner };
        RingBuf { reader: reader, writer: writer }
    }
}

#[derive(Debug)]
pub struct RingBufReader {
    inner: Arc<RingBufInner>,
}

#[derive(Debug)]
pub struct RingBufWriter {
    inner: Arc<RingBufInner>,
}

#[derive(Debug)]
struct RingBufInner {
    buf: *mut u8,
    capacity: usize,
    head: AtomicUsize, // write to head
    tail: AtomicUsize, // read from tail
    closed: AtomicBool, // if reader has been dropped
}

impl RingBufInner {
    fn new(capacity: usize) -> RingBufInner {
        let capacity = max(capacity, 16).next_power_of_two();
        RingBufInner {
            buf: unsafe {
                let mut buf_ptr = ptr::null_mut();
                libc::posix_memalign(&mut buf_ptr, 16, capacity);
                assert!(buf_ptr != ptr::null_mut());
                buf_ptr as *mut u8
            },
            capacity: capacity,
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
            closed: AtomicBool::new(false),
        }
    }

    fn get_mask(&self) -> usize {
        self.capacity - 1 // Note that capacity is a power of two
    }

    fn get_head(&self) -> usize {
        self.head.load(Ordering::SeqCst)
    }

    fn get_tail(&self) -> usize {
        self.tail.load(Ordering::SeqCst)
    }

    fn set_head(&self, new_head: usize) {
        self.head.store(new_head, Ordering::SeqCst)
    }

    fn set_tail(&self, new_tail: usize) {
        self.tail.store(new_tail, Ordering::SeqCst)
    }

    fn is_closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }

    fn close(&self) {
        self.closed.store(true, Ordering::SeqCst);
    }

    unsafe fn read_at(&self, pos: usize, dst_buf: &mut [u8]) {
        let dst_ptr = dst_buf.as_mut_ptr();
        let dst_len = dst_buf.len();
        let src_ptr = self.buf.offset(pos as isize);
        unsafe {
            src_ptr.copy_to_nonoverlapping(dst_ptr, dst_len);
        }
    }

    unsafe fn write_at(&self, pos: usize, src_buf: &[u8]) {
        let src_ptr = src_buf.as_ptr();
        let src_len = src_buf.len();
        let dst_ptr = self.buf.offset(pos as isize);
        unsafe {
            dst_ptr.copy_from_nonoverlapping(src_ptr, src_len);
        }
    }
}

impl Drop for RingBufInner {
    fn drop(&mut self) {
        unsafe { libc::free(self.buf as *mut c_void) }
    }
}


impl RingBufReader {
    pub fn read(&self, buf: &mut [u8]) -> Result<usize, Error> {
        let mut tail = self.inner.get_tail();
        let mut buf_remain = buf.len();
        let mut buf_pos = 0;
        while buf_remain > 0 {
            let head = self.inner.get_head();

            let read_nbytes = {
                let may_read_nbytes = if tail <= head {
                    head - tail
                } else {
                    self.inner.capacity - tail
                };
                if may_read_nbytes == 0 { break; }

                min(may_read_nbytes, buf_remain)
            };

            let dst_buf = &mut buf[buf_pos..(buf_pos + read_nbytes)];
            unsafe {
                self.inner.read_at(tail, dst_buf);
            }

            tail = (tail + read_nbytes) & self.inner.get_mask();
            self.inner.set_tail(tail);

            buf_pos += read_nbytes;
            buf_remain -= read_nbytes;
        }
        Ok(buf_pos)
    }
}

impl Drop for RingBufReader {
    fn drop(&mut self) {
        // So the writer knows when a reader is finished
        self.inner.close();
    }
}


impl RingBufWriter {
    pub fn write(&self, buf: &[u8]) -> Result<usize, Error> {
        if self.inner.is_closed() {
            return errno!(EPIPE, "Reader has been closed");
        }

        let mut head = self.inner.get_head();
        let mut buf_remain = buf.len();
        let mut buf_pos = 0;
        while buf_remain > 0 {
            let tail = self.inner.get_tail();

            let write_nbytes = {
                let may_write_nbytes = if tail <= head {
                    self.inner.capacity - head
                } else {
                    tail - head - 1
                };
                if may_write_nbytes == 0 { break; }

                min(may_write_nbytes, buf_remain)
            };

            let src_buf = &buf[buf_pos..(buf_pos + write_nbytes)];
            unsafe {
                self.inner.write_at(head, src_buf);
            }

            head = (head + write_nbytes) & self.inner.get_mask();
            self.inner.set_head(head);

            buf_pos += write_nbytes;
            buf_remain -= write_nbytes;
        }
        Ok(buf_pos)
    }
}
