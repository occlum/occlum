use std::sync::atomic::{AtomicBool, Ordering};

use atomic::Atomic;
use ringbuf::{Consumer as RbConsumer, Producer as RbProducer, RingBuffer};

use crate::event::{Events, Observer, Pollee, Poller};
use crate::file::{AccessMode, File, StatusFlags};
use crate::prelude::*;

/// A unidirectional communication channel, intended to implement IPC, e.g., pipe,
/// unix domain sockets, etc.
#[derive(Debug)]
pub struct Channel<T> {
    producer: Producer<T>,
    consumer: Consumer<T>,
}

#[derive(Debug)]
pub struct Producer<T> {
    common: Arc<Common<T>>,
}

#[derive(Debug)]
pub struct Consumer<T> {
    common: Arc<Common<T>>,
}

#[derive(Debug)]
struct Common<T> {
    producer: EndPoint<RbProducer<T>>,
    consumer: EndPoint<RbConsumer<T>>,
    event_lock: Mutex<()>,
}

struct EndPoint<T> {
    ringbuf: Mutex<T>,
    pollee: Pollee,
    is_shutdown: AtomicBool,
    flags: Atomic<StatusFlags>,
}

impl<T> Channel<T> {
    pub fn with_capacity(capacity: usize) -> Result<Self> {
        Self::with_capacity_and_flags(capacity, StatusFlags::empty())
    }

    pub fn with_capacity_and_flags(capacity: usize, flags: StatusFlags) -> Result<Self> {
        let common = Arc::new(Common::with_capacity_and_flags(capacity, flags)?);
        let producer = Producer {
            common: common.clone(),
        };
        let consumer = Consumer { common: common };
        Ok(Self { producer, consumer })
    }

    pub fn split(self) -> (Producer<T>, Consumer<T>) {
        let Self { producer, consumer } = self;
        (producer, consumer)
    }

    pub fn producer(&self) -> &Producer<T> {
        &self.producer
    }

    pub fn consumer(&self) -> &Consumer<T> {
        &self.consumer
    }

    pub fn capacity(&self) -> usize {
        self.producer.common.capacity()
    }

    pub fn push(&self, item: T) -> Result<()> {
        self.producer
            .common
            .producer
            .ringbuf
            .lock()
            .push(item)
            .map_err(|_| errno!(EAGAIN, "push ring buffer failure"))?;

        self.consumer
            .common
            .consumer
            .pollee()
            .add_events(Events::IN);
        Ok(())
    }

    /// Pop an item out of the channel.
    pub async fn pop(&self) -> Result<T> {
        // Init the poller only when needed
        let mut poller = None;
        loop {
            let ret = self.consumer.common.consumer.ringbuf.lock().pop();
            if let Some(item) = ret {
                return Ok(item);
            }

            if self.consumer.is_nonblocking() {
                return_errno!(EAGAIN, "no connections are present to be accepted");
            }

            // Ensure the poller is initialized
            if poller.is_none() {
                poller = Some(Poller::new());
            }
            // Wait for interesting events by polling
            let mask = Events::IN;
            let events = self
                .consumer
                .common
                .consumer
                .pollee()
                .poll(mask, poller.as_mut());
            if events.is_empty() {
                poller.as_ref().unwrap().wait().await?;
            }
        }
    }
}

impl<T> Common<T> {
    pub fn with_capacity_and_flags(capacity: usize, flags: StatusFlags) -> Result<Self> {
        check_status_flags(flags)?;

        if capacity == 0 {
            return_errno!(EINVAL, "capacity cannot be zero");
        }

        let rb: RingBuffer<T> = RingBuffer::new(capacity);
        let (rb_producer, rb_consumer) = rb.split();

        let producer = EndPoint::new(rb_producer, Events::OUT, flags);
        let consumer = EndPoint::new(rb_consumer, Events::empty(), flags);

        let event_lock = Mutex::new(());

        Ok(Self {
            producer,
            consumer,
            event_lock,
        })
    }

    pub fn lock_event(&self) -> MutexGuard<()> {
        self.event_lock.lock()
    }

    pub fn capacity(&self) -> usize {
        self.producer.ringbuf.lock().capacity()
    }
}

impl<T> EndPoint<T> {
    pub fn new(ringbuf: T, init_events: Events, flags: StatusFlags) -> Self {
        Self {
            ringbuf: Mutex::new(ringbuf),
            pollee: Pollee::new(init_events),
            is_shutdown: AtomicBool::new(false),
            flags: Atomic::new(flags),
        }
    }

    pub fn ringbuf(&self) -> MutexGuard<T> {
        self.ringbuf.lock()
    }

    pub fn pollee(&self) -> &Pollee {
        &self.pollee
    }

    pub fn is_shutdown(&self) -> bool {
        self.is_shutdown.load(Ordering::Acquire)
    }

    pub fn shutdown(&self) {
        self.is_shutdown.store(true, Ordering::Release)
    }
}

impl<T> std::fmt::Debug for EndPoint<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EndPoint")
            .field("ringbuf", &"..")
            .field("pollee", self.pollee())
            .field("is_shutdown", &self.is_shutdown())
            .finish()
    }
}

impl<T> Producer<T> {
    fn this_end(&self) -> &EndPoint<RbProducer<T>> {
        &self.common.producer
    }

    fn peer_end(&self) -> &EndPoint<RbConsumer<T>> {
        &self.common.consumer
    }

    pub fn peer_is_shutdown(&self) -> bool {
        self.peer_end().is_shutdown()
    }

    fn update_pollee(&self) {
        let this_end = self.this_end();
        let peer_end = self.peer_end();

        // Update the event of pollee in a critical region so that pollee
        // always reflects the _true_ state of the underlying ring buffer
        // regardless of any race conditions.
        self.common.lock_event();

        let rb = this_end.ringbuf();
        if rb.is_full() {
            this_end.pollee().del_events(Events::OUT);
        }
        if !rb.is_empty() {
            peer_end.pollee().add_events(Events::IN);
        }
    }

    pub fn shutdown(&self) {
        self.common.producer.shutdown()
    }

    pub fn is_shutdown(&self) -> bool {
        self.common.producer.is_shutdown()
    }

    pub fn status_flags(&self) -> StatusFlags {
        self.this_end().flags.load(Ordering::Relaxed)
    }

    pub fn set_status_flags(&self, new_status: StatusFlags) -> Result<()> {
        check_status_flags(new_status)?;
        self.this_end().flags.store(new_status, Ordering::Relaxed);
        Ok(())
    }

    pub fn is_nonblocking(&self) -> bool {
        self.status_flags().contains(StatusFlags::O_NONBLOCK)
    }

    pub fn pollee(&self) -> &Pollee {
        self.common.producer.pollee()
    }
}

impl Producer<u8> {
    pub async fn writev_async(&self, bufs: &[&[u8]]) -> Result<usize> {
        let total_len: usize = bufs.iter().map(|buf| buf.len()).sum();
        if total_len == 0 {
            return Ok(0);
        }

        let mut poller = None;
        loop {
            // Attempt to write
            let ret = self.try_writev(bufs);
            if !ret.has_errno(EAGAIN) {
                return ret;
            }

            if self.is_nonblocking() {
                return_errno!(EAGAIN, "buffer is full");
            }

            // Wait for interesting events by polling
            if poller.is_none() {
                poller = Some(Poller::new());
            }
            let mask = Events::OUT;
            let events = self.pollee().poll(mask, poller.as_mut());
            if events.is_empty() {
                poller.as_ref().unwrap().wait().await?;
            }
        }
    }

    fn try_writev(&self, bufs: &[&[u8]]) -> Result<usize> {
        let mut total_nbytes = 0;
        let mut errno = EAGAIN;
        for buf in bufs {
            match self.write(buf) {
                Ok(nbytes) => {
                    total_nbytes += nbytes;
                    if nbytes < buf.len() {
                        break;
                    }
                }
                Err(e) => {
                    errno = e.errno();
                    break;
                }
            }
        }

        if total_nbytes > 0 {
            return Ok(total_nbytes);
        }

        return_errno!(errno, "error when writing");
    }
}

impl File for Producer<u8> {
    fn write(&self, buf: &[u8]) -> Result<usize> {
        let this_end = self.this_end();
        let peer_end = self.peer_end();

        if this_end.is_shutdown() || peer_end.is_shutdown() {
            return_errno!(EPIPE, "");
        }

        if buf.len() == 0 {
            return Ok(0);
        }

        let nbytes = {
            let mut rb = this_end.ringbuf();
            let nbytes = rb.push_slice(buf);
            nbytes
        };

        self.update_pollee();

        if nbytes > 0 {
            Ok(nbytes)
        } else {
            return_errno!(EAGAIN, "try write later");
        }
    }

    // TODO: implement writev

    fn poll(&self, mask: Events, poller: Option<&mut Poller>) -> Events {
        self.this_end().pollee().poll(mask, poller)
    }

    fn register_observer(&self, observer: Arc<dyn Observer>, mask: Events) -> Result<()> {
        self.this_end().pollee().register_observer(observer, mask);
        Ok(())
    }

    fn unregister_observer(&self, observer: &Arc<dyn Observer>) -> Result<Arc<dyn Observer>> {
        self.this_end()
            .pollee()
            .unregister_observer(observer)
            .ok_or_else(|| errno!(ENOENT, "the observer is not registered"))
    }

    fn status_flags(&self) -> StatusFlags {
        self.status_flags()
    }

    fn set_status_flags(&self, new_status: StatusFlags) -> Result<()> {
        self.set_status_flags(new_status)
    }

    fn access_mode(&self) -> AccessMode {
        AccessMode::O_WRONLY
    }
}

impl<T> Drop for Producer<T> {
    fn drop(&mut self) {
        // This is called when the writer end is closed (not shutdown)
        let this_end = self.this_end();
        // man poll:
        // When reading from a channel such as a pipe or a stream socket, POLLHUP merely indicates that the peer
        // closed its end of the channel.
        let mut revents = Events::HUP;

        this_end.shutdown();

        self.common.lock_event();

        let rb = this_end.ringbuf();
        if !rb.is_empty() {
            revents |= Events::IN;
        }

        self.peer_end().pollee().add_events(revents);
    }
}

impl<T> Consumer<T> {
    fn this_end(&self) -> &EndPoint<RbConsumer<T>> {
        &self.common.consumer
    }

    fn peer_end(&self) -> &EndPoint<RbProducer<T>> {
        &self.common.producer
    }

    fn update_pollee(&self) {
        let this_end = self.this_end();
        let peer_end = self.peer_end();

        // Update the event of pollee in a critical region so that pollee
        // always reflects the _true_ state of the underlying ring buffer
        // regardless of any race conditions.
        self.common.lock_event();

        let rb = this_end.ringbuf();
        if rb.is_empty() {
            this_end.pollee().del_events(Events::IN);
        }
        if !rb.is_full() {
            peer_end.pollee().add_events(Events::OUT);
        }
    }

    // Get the length of data stored in the buffer
    pub fn ready_len(&self) -> usize {
        let this_end = self.this_end();
        let rb = this_end.ringbuf();
        rb.len()
    }

    pub fn shutdown(&self) {
        self.common.consumer.shutdown()
    }

    pub fn is_shutdown(&self) -> bool {
        self.common.consumer.is_shutdown()
    }

    pub fn status_flags(&self) -> StatusFlags {
        self.this_end().flags.load(Ordering::Relaxed)
    }

    pub fn set_status_flags(&self, new_status: StatusFlags) -> Result<()> {
        check_status_flags(new_status)?;
        self.this_end().flags.store(new_status, Ordering::Relaxed);
        Ok(())
    }

    pub fn is_nonblocking(&self) -> bool {
        self.status_flags().contains(StatusFlags::O_NONBLOCK)
    }

    pub fn pollee(&self) -> &Pollee {
        self.common.consumer.pollee()
    }
}

impl Consumer<u8> {
    pub async fn readv_async(&self, bufs: &mut [&mut [u8]]) -> Result<usize> {
        let total_len: usize = bufs.iter().map(|buf| buf.len()).sum();
        if total_len == 0 {
            return Ok(0);
        }

        let mut poller = None;
        loop {
            // Attempt to read
            let ret = self.try_readv(bufs);
            if !ret.has_errno(EAGAIN) {
                return ret;
            }

            if self.is_nonblocking() {
                return_errno!(EAGAIN, "no data are present to be received");
            }

            if poller.is_none() {
                poller = Some(Poller::new());
            }
            let mask = Events::IN;
            let events = self.pollee().poll(mask, poller.as_mut());
            if events.is_empty() {
                poller.as_ref().unwrap().wait().await?;
            }
        }
    }

    fn try_readv(&self, bufs: &mut [&mut [u8]]) -> Result<usize> {
        let mut total_nbytes = 0;
        let mut errno = EAGAIN;
        for buf in bufs {
            match self.read(buf) {
                Ok(nbytes) => {
                    total_nbytes += nbytes;
                    if nbytes < buf.len() {
                        break;
                    }
                }
                Err(e) => {
                    errno = e.errno();
                    break;
                }
            }
        }

        if total_nbytes > 0 {
            return Ok(total_nbytes);
        }

        return_errno!(errno, "error when reading");
    }
}

impl File for Consumer<u8> {
    fn read(&self, buf: &mut [u8]) -> Result<usize> {
        let this_end = self.this_end();
        let peer_end = self.peer_end();

        if this_end.is_shutdown() {
            return_errno!(EPIPE, "");
        }

        if buf.len() == 0 {
            return Ok(0);
        }

        let nbytes = {
            let mut rb = this_end.ringbuf();
            let nbytes = rb.pop_slice(buf);
            nbytes
        };

        self.update_pollee();

        if peer_end.is_shutdown() {
            return Ok(nbytes);
        }

        if nbytes > 0 {
            Ok(nbytes)
        } else {
            return_errno!(EAGAIN, "try read later");
        }
    }

    // TODO: implement readv

    fn poll(&self, mask: Events, poller: Option<&mut Poller>) -> Events {
        self.this_end().pollee().poll(mask, poller)
    }

    fn register_observer(&self, observer: Arc<dyn Observer>, mask: Events) -> Result<()> {
        self.this_end().pollee().register_observer(observer, mask);
        Ok(())
    }

    fn unregister_observer(&self, observer: &Arc<dyn Observer>) -> Result<Arc<dyn Observer>> {
        self.this_end()
            .pollee()
            .unregister_observer(observer)
            .ok_or_else(|| errno!(ENOENT, "the observer is not registered"))
    }

    fn status_flags(&self) -> StatusFlags {
        self.status_flags()
    }

    fn set_status_flags(&self, new_status: StatusFlags) -> Result<()> {
        self.set_status_flags(new_status)
    }

    fn access_mode(&self) -> AccessMode {
        AccessMode::O_RDONLY
    }
}

impl<T> Drop for Consumer<T> {
    fn drop(&mut self) {
        // This is called when the reader end is closed (not shutdown)
        let this_end = self.this_end();
        // Man poll:
        // POLLERR is also set for a file descriptor referring to the write end of a pipe when the read end has
        // been closed.
        let mut revents = Events::ERR;

        this_end.shutdown();

        self.common.lock_event();

        let rb = this_end.ringbuf();
        if !rb.is_full() {
            // poll reacts to event happening on the file descriptors. when file is closed,
            // there will be no fd anymore and the events are not updated.
            revents |= Events::OUT;
        }

        self.peer_end().pollee().add_events(revents);
    }
}

#[cfg(test)]
mod tests {
    use std::fmt::Debug;
    use std::sync::Arc;

    use super::*;
    use crate::file::{Async, File};

    #[test]
    fn transfer_data_with_small_buf() {
        async_rt::task::block_on(async {
            const TOTAL_NBYTES: usize = 4 * 1024 * 1024;
            const CHANNEL_CAPACITY: usize = 4 * 1024;
            const BUF_SIZE: usize = 128;
            do_transfer_data(TOTAL_NBYTES, CHANNEL_CAPACITY, BUF_SIZE).await;
        });
    }

    #[test]
    fn transfer_data_with_big_buf() {
        async_rt::task::block_on(async {
            const TOTAL_NBYTES: usize = 16 * 1024 * 1024;
            const CHANNEL_CAPACITY: usize = 4 * 1024;
            const BUF_SIZE: usize = 6 * 1024;
            do_transfer_data(TOTAL_NBYTES, CHANNEL_CAPACITY, BUF_SIZE).await;
        });
    }

    async fn do_transfer_data(total_nbytes: usize, channel_capacity: usize, buf_size: usize) {
        let channel = Channel::with_capacity(channel_capacity).unwrap();
        let (producer, consumer) = channel.split();
        let producer = Async::new(producer);
        let consumer = Async::new(consumer);

        let producer_handle = async_rt::task::spawn(async move {
            let mut buf = Vec::with_capacity(buf_size);
            unsafe {
                buf.set_len(buf.capacity());
            }

            let mut sofar_nbytes = 0;
            while sofar_nbytes < total_nbytes {
                let nbytes = producer.write(buf.as_slice()).await.unwrap();
                sofar_nbytes += nbytes;
            }
        });

        let consumer_handle = async_rt::task::spawn(async move {
            let mut buf = Vec::with_capacity(buf_size);
            unsafe {
                buf.set_len(buf.capacity());
            }

            let mut sofar_nbytes = 0;
            while sofar_nbytes < total_nbytes {
                let nbytes = consumer.read(buf.as_mut_slice()).await.unwrap();
                sofar_nbytes += nbytes;
            }
        });

        producer_handle.await;
        consumer_handle.await;
    }

    #[test]
    fn poll() {
        const BUF_LEN: usize = 4 * 1024;
        const CHANNEL_CAPACITY: usize = 2 * BUF_LEN;

        let mask = Events::all();

        let mut buf = Vec::with_capacity(BUF_LEN);
        unsafe {
            buf.set_len(BUF_LEN);
        }

        let channel = Channel::with_capacity(CHANNEL_CAPACITY).unwrap();
        let (producer, consumer) = channel.split();

        // Initial events
        assert!(producer.poll(mask, None) == Events::OUT);
        assert!(consumer.poll(mask, None) == Events::empty());

        // First write
        producer.write(&buf[..BUF_LEN]);
        assert!(producer.poll(mask, None) == Events::OUT);
        assert!(consumer.poll(mask, None) == Events::IN);

        // First read, but only half of the avail data
        consumer.read(&mut buf[..BUF_LEN / 2]);
        assert!(producer.poll(mask, None) == Events::OUT);
        assert!(consumer.poll(mask, None) == Events::IN);

        // Second read, consume the rest of avail data
        consumer.read(&mut buf[..BUF_LEN / 2]);
        assert!(producer.poll(mask, None) == Events::OUT);
        assert!(consumer.poll(mask, None) == Events::empty());

        // Second and third write, filling up the underlying buffer
        producer.write(&buf[..BUF_LEN]);
        producer.write(&buf[..BUF_LEN]);
        assert!(producer.poll(mask, None) == Events::empty());
        assert!(consumer.poll(mask, None) == Events::IN);
    }
}

fn check_status_flags(flags: StatusFlags) -> Result<()> {
    let valid_flags: StatusFlags = StatusFlags::O_NONBLOCK | StatusFlags::O_DIRECT;
    if !valid_flags.contains(flags) {
        return_errno!(EINVAL, "invalid flags");
    }
    if flags.contains(StatusFlags::O_DIRECT) {
        return_errno!(EINVAL, "O_DIRECT is not supported");
    }
    Ok(())
}
