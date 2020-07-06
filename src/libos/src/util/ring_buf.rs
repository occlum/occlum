use alloc::alloc::{alloc, dealloc, Layout};

use crate::net::{
    clear_notifier_status, notify_thread, wait_for_notification, IoEvent, PollEventFlags,
};
use std::cmp::{max, min};
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use super::*;
use ringbuf::{Consumer, Producer, RingBuffer};

pub fn ring_buffer(capacity: usize) -> Result<(RingBufReader, RingBufWriter)> {
    let meta = RingBufMeta::new();
    let buffer = RingBuffer::<u8>::new(capacity);
    let (producer, consumer) = buffer.split();
    let meta_ref = Arc::new(meta);

    let reader = RingBufReader {
        inner: consumer,
        buffer: meta_ref.clone(),
    };
    let writer = RingBufWriter {
        inner: producer,
        buffer: meta_ref,
    };
    Ok((reader, writer))
}

struct RingBufMeta {
    lock: Arc<SgxMutex<bool>>, // lock for the synchronization of reader and writer
    reader_closed: AtomicBool, // if reader has been dropped
    writer_closed: AtomicBool, // if writer has been dropped
    reader_wait_queue: SgxMutex<HashMap<pid_t, IoEvent>>,
    writer_wait_queue: SgxMutex<HashMap<pid_t, IoEvent>>,
    // TODO: support O_ASYNC and O_DIRECT in ringbuffer
    blocking_read: AtomicBool,  // if the read is blocking
    blocking_write: AtomicBool, // if the write is blocking
}

impl RingBufMeta {
    pub fn new() -> RingBufMeta {
        Self {
            lock: Arc::new(SgxMutex::new(true)),
            reader_closed: AtomicBool::new(false),
            writer_closed: AtomicBool::new(false),
            reader_wait_queue: SgxMutex::new(HashMap::new()),
            writer_wait_queue: SgxMutex::new(HashMap::new()),
            blocking_read: AtomicBool::new(true),
            blocking_write: AtomicBool::new(true),
        }
    }

    pub fn is_reader_closed(&self) -> bool {
        self.reader_closed.load(Ordering::SeqCst)
    }

    pub fn close_reader(&self) {
        self.reader_closed.store(true, Ordering::SeqCst);
    }

    pub fn is_writer_closed(&self) -> bool {
        self.writer_closed.load(Ordering::SeqCst)
    }

    pub fn close_writer(&self) {
        self.writer_closed.store(true, Ordering::SeqCst);
    }

    pub fn reader_wait_queue(&self) -> &SgxMutex<HashMap<pid_t, IoEvent>> {
        &self.reader_wait_queue
    }

    pub fn writer_wait_queue(&self) -> &SgxMutex<HashMap<pid_t, IoEvent>> {
        &self.writer_wait_queue
    }

    pub fn enqueue_reader_event(&self, event: IoEvent) -> Result<()> {
        self.reader_wait_queue
            .lock()
            .unwrap()
            .insert(current!().tid(), event);
        Ok(())
    }

    pub fn dequeue_reader_event(&self) -> Result<()> {
        self.reader_wait_queue
            .lock()
            .unwrap()
            .remove(&current!().tid())
            .unwrap();
        Ok(())
    }

    pub fn enqueue_writer_event(&self, event: IoEvent) -> Result<()> {
        self.writer_wait_queue
            .lock()
            .unwrap()
            .insert(current!().tid(), event);
        Ok(())
    }

    pub fn dequeue_writer_event(&self) -> Result<()> {
        self.writer_wait_queue
            .lock()
            .unwrap()
            .remove(&current!().tid())
            .unwrap();
        Ok(())
    }

    pub fn blocking_read(&self) -> bool {
        self.blocking_read.load(Ordering::SeqCst)
    }

    pub fn set_non_blocking_read(&self) {
        self.blocking_read.store(false, Ordering::SeqCst);
    }

    pub fn set_blocking_read(&self) {
        self.blocking_read.store(true, Ordering::SeqCst);
    }

    pub fn blocking_write(&self) -> bool {
        self.blocking_write.load(Ordering::SeqCst)
    }

    pub fn set_non_blocking_write(&self) {
        self.blocking_write.store(false, Ordering::SeqCst);
    }

    pub fn set_blocking_write(&self) {
        self.blocking_write.store(true, Ordering::SeqCst);
    }
}

pub struct RingBufReader {
    inner: Consumer<u8>,
    buffer: Arc<RingBufMeta>,
}

impl RingBufReader {
    pub fn can_read(&self) -> bool {
        self.bytes_to_read() != 0
    }

    pub fn read_from_buffer(&mut self, buffer: &mut [u8]) -> Result<usize> {
        self.read(Some(buffer), None)
    }

    pub fn read_from_vector(&mut self, buffers: &mut [&mut [u8]]) -> Result<usize> {
        self.read(None, Some(buffers))
    }

    fn read(
        &mut self,
        buffer: Option<&mut [u8]>,
        buffers: Option<&mut [&mut [u8]]>,
    ) -> Result<usize> {
        assert!(buffer.is_some() ^ buffers.is_some());
        // In case of write after can_read is false
        let lock_ref = self.buffer.lock.clone();
        let lock_holder = lock_ref.lock();

        if self.can_read() {
            let count = if buffer.is_some() {
                self.inner.pop_slice(buffer.unwrap())
            } else {
                self.pop_slices(buffers.unwrap())
            };
            assert!(count > 0);
            self.read_end();
            Ok(count)
        } else {
            if self.is_peer_closed() {
                return Ok(0);
            }

            if !self.buffer.blocking_read() {
                return_errno!(EAGAIN, "No data to read");
            } else {
                // Clear the status of notifier before enqueue
                clear_notifier_status(current!().tid())?;
                self.enqueue_event(IoEvent::BlockingRead)?;
                drop(lock_holder);
                drop(lock_ref);
                let ret = wait_for_notification();
                self.dequeue_event()?;
                ret?;

                let lock_ref = self.buffer.lock.clone();
                let lock_holder = lock_ref.lock();
                let count = if buffer.is_some() {
                    self.inner.pop_slice(buffer.unwrap())
                } else {
                    self.pop_slices(buffers.unwrap())
                };

                if count > 0 {
                    self.read_end()?;
                } else {
                    assert!(self.is_peer_closed());
                }
                Ok(count)
            }
        }
    }

    fn pop_slices(&mut self, buffers: &mut [&mut [u8]]) -> usize {
        let mut total = 0;
        for buf in buffers {
            let count = self.inner.pop_slice(buf);
            total += count;
            if count < buf.len() {
                break;
            }
        }
        total
    }

    pub fn bytes_to_read(&self) -> usize {
        self.inner.len()
    }

    fn read_end(&self) -> Result<()> {
        for (tid, event) in &*self.buffer.writer_wait_queue().lock().unwrap() {
            match event {
                IoEvent::Poll(poll_events) => {
                    if !(poll_events.events()
                        & (PollEventFlags::POLLOUT | PollEventFlags::POLLWRNORM))
                        .is_empty()
                    {
                        notify_thread(*tid)?;
                    }
                }
                IoEvent::Epoll(epoll_file) => unimplemented!(),
                IoEvent::BlockingRead => unreachable!(),
                IoEvent::BlockingWrite => notify_thread(*tid)?,
            }
        }
        Ok(())
    }

    pub fn is_peer_closed(&self) -> bool {
        self.buffer.is_writer_closed()
    }

    pub fn enqueue_event(&self, event: IoEvent) -> Result<()> {
        self.buffer.enqueue_reader_event(event)
    }

    pub fn dequeue_event(&self) -> Result<()> {
        self.buffer.dequeue_reader_event()
    }

    pub fn set_non_blocking(&self) {
        self.buffer.set_non_blocking_read()
    }

    pub fn set_blocking(&self) {
        self.buffer.set_blocking_read()
    }

    fn before_drop(&self) {
        for (tid, event) in &*self.buffer.writer_wait_queue().lock().unwrap() {
            match event {
                IoEvent::Poll(_) | IoEvent::BlockingWrite => notify_thread(*tid).unwrap(),
                IoEvent::Epoll(epoll_file) => unimplemented!(),
                IoEvent::BlockingRead => unreachable!(),
            }
        }
    }
}

impl Drop for RingBufReader {
    fn drop(&mut self) {
        debug!("reader drop");
        self.buffer.close_reader();
        if self.buffer.blocking_write() {
            self.before_drop();
        }
    }
}

pub struct RingBufWriter {
    inner: Producer<u8>,
    buffer: Arc<RingBufMeta>,
}

impl RingBufWriter {
    pub fn write_to_buffer(&mut self, buffer: &[u8]) -> Result<usize> {
        self.write(Some(buffer), None)
    }

    pub fn write_to_vector(&mut self, buffers: &[&[u8]]) -> Result<usize> {
        self.write(None, Some(buffers))
    }

    fn write(&mut self, buffer: Option<&[u8]>, buffers: Option<&[&[u8]]>) -> Result<usize> {
        assert!(buffer.is_some() ^ buffers.is_some());

        // TODO: send SIGPIPE to the caller
        if self.is_peer_closed() {
            return_errno!(EPIPE, "reader side is closed");
        }

        // In case of read after can_write is false
        let lock_ref = self.buffer.lock.clone();
        let lock_holder = lock_ref.lock();

        if self.can_write() {
            let count = if buffer.is_some() {
                self.inner.push_slice(buffer.unwrap())
            } else {
                self.push_slices(buffers.unwrap())
            };
            assert!(count > 0);
            self.write_end();
            Ok(count)
        } else {
            if !self.buffer.blocking_write() {
                return_errno!(EAGAIN, "No space to write");
            }

            // Clear the status of notifier before enqueue
            clear_notifier_status(current!().tid());
            self.enqueue_event(IoEvent::BlockingWrite)?;
            drop(lock_holder);
            drop(lock_ref);
            let ret = wait_for_notification();
            self.dequeue_event()?;
            ret?;

            let lock_ref = self.buffer.lock.clone();
            let lock_holder = lock_ref.lock();
            let count = if buffer.is_some() {
                self.inner.push_slice(buffer.unwrap())
            } else {
                self.push_slices(buffers.unwrap())
            };

            if count > 0 {
                self.write_end();
                Ok(count)
            } else {
                return_errno!(EPIPE, "reader side is closed");
            }
        }
    }

    fn write_end(&self) -> Result<()> {
        for (tid, event) in &*self.buffer.reader_wait_queue().lock().unwrap() {
            match event {
                IoEvent::Poll(poll_events) => {
                    if !(poll_events.events()
                        & (PollEventFlags::POLLIN | PollEventFlags::POLLRDNORM))
                        .is_empty()
                    {
                        notify_thread(*tid)?;
                    }
                }
                IoEvent::Epoll(epoll_file) => unimplemented!(),
                IoEvent::BlockingRead => notify_thread(*tid)?,
                IoEvent::BlockingWrite => unreachable!(),
            }
        }
        Ok(())
    }

    fn push_slices(&mut self, buffers: &[&[u8]]) -> usize {
        let mut total = 0;
        for buf in buffers {
            let count = self.inner.push_slice(buf);
            total += count;
            if count < buf.len() {
                break;
            }
        }
        total
    }

    pub fn can_write(&self) -> bool {
        !self.inner.is_full()
    }

    pub fn is_peer_closed(&self) -> bool {
        self.buffer.is_reader_closed()
    }

    pub fn enqueue_event(&self, event: IoEvent) -> Result<()> {
        self.buffer.enqueue_writer_event(event)
    }

    pub fn dequeue_event(&self) -> Result<()> {
        self.buffer.dequeue_writer_event()
    }

    pub fn set_non_blocking(&self) {
        self.buffer.set_non_blocking_write()
    }

    pub fn set_blocking(&self) {
        self.buffer.set_blocking_write()
    }

    fn before_drop(&self) {
        for (tid, event) in &*self.buffer.reader_wait_queue().lock().unwrap() {
            match event {
                IoEvent::Poll(_) | IoEvent::BlockingRead => {
                    notify_thread(*tid).unwrap();
                }
                IoEvent::Epoll(epoll_file) => unimplemented!(),
                IoEvent::BlockingWrite => unreachable!(),
            }
        }
    }
}

impl Drop for RingBufWriter {
    fn drop(&mut self) {
        debug!("writer drop");
        self.buffer.close_writer();
        if self.buffer.blocking_read() {
            self.before_drop();
        }
    }
}
