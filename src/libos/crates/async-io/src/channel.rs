use std::any::Any;
use std::sync::atomic::{AtomicBool, Ordering};

use ringbuf::{Consumer as RbConsumer, Producer as RbProducer, RingBuffer};

use crate::file::File;
use crate::poll::{Events, Pollee, Poller};
use crate::prelude::*;

/// A unidirectional communication channel, intended to implement IPC, e.g., pipe,
/// unix domain sockets, etc.
#[derive(Debug)]
pub struct Channel {
    producer: Producer,
    consumer: Consumer,
}

#[derive(Debug)]
pub struct Producer {
    common: Arc<Common>,
}

#[derive(Debug)]
pub struct Consumer {
    common: Arc<Common>,
}

#[derive(Debug)]
struct Common {
    producer: EndPoint<RbProducer<u8>>,
    consumer: EndPoint<RbConsumer<u8>>,
    event_lock: Mutex<()>,
}

struct EndPoint<T> {
    ringbuf: Mutex<T>,
    pollee: Pollee,
    is_shutdown: AtomicBool,
}

impl Channel {
    pub fn with_capacity(capacity: usize) -> Result<Self> {
        let common = Arc::new(Common::with_capacity(capacity)?);
        let producer = Producer {
            common: common.clone(),
        };
        let consumer = Consumer { common: common };
        Ok(Self { producer, consumer })
    }

    pub fn split(self) -> (Producer, Consumer) {
        let Self { producer, consumer } = self;
        (producer, consumer)
    }

    pub fn producer(&self) -> &Producer {
        &self.producer
    }

    pub fn consumer(&self) -> &Consumer {
        &self.consumer
    }
}

impl Common {
    pub fn with_capacity(capacity: usize) -> Result<Self> {
        if capacity == 0 {
            return_errno!(EINVAL, "capacity cannot be zero");
        }

        let rb: RingBuffer<u8> = RingBuffer::new(capacity);
        let (rb_producer, rb_consumer) = rb.split();

        let producer = EndPoint::new(rb_producer, Events::OUT);
        let consumer = EndPoint::new(rb_consumer, Events::empty());

        let event_lock = Mutex::new(());

        Ok(Self {
            producer,
            consumer,
            event_lock,
        })
    }

    pub fn lock_event(&self) -> MutexGuard<()> {
        self.event_lock.lock().unwrap()
    }
}

impl<T> EndPoint<T> {
    pub fn new(ringbuf: T, init_events: Events) -> Self {
        Self {
            ringbuf: Mutex::new(ringbuf),
            pollee: Pollee::new(init_events),
            is_shutdown: AtomicBool::new(false),
        }
    }

    pub fn ringbuf(&self) -> MutexGuard<T> {
        self.ringbuf.lock().unwrap()
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

impl Producer {
    fn this_end(&self) -> &EndPoint<RbProducer<u8>> {
        &self.common.producer
    }

    fn peer_end(&self) -> &EndPoint<RbConsumer<u8>> {
        &self.common.consumer
    }

    fn update_pollee(&self) {
        let this_end = self.this_end();
        let peer_end = self.peer_end();

        // Update the event of pollee in a critical region so that pollee
        // always reflects the _true_ state of the underlying ring buffer
        // regardless of any race conditions.
        let event_lock = self.common.lock_event();

        let rb = this_end.ringbuf();
        if rb.is_full() {
            this_end.pollee().del_events(Events::OUT);
        }
        if !rb.is_empty() {
            peer_end.pollee().add_events(Events::IN);
        }
    }
}

impl File for Producer {
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

    fn poll_by(&self, mask: Events, poller: Option<&mut Poller>) -> Events {
        self.this_end().pollee().poll_by(mask, poller)
    }

    fn as_any(&self) -> &dyn Any {
        self as &dyn Any
    }
}

impl Drop for Producer {
    fn drop(&mut self) {
        self.peer_end()
            .pollee()
            .add_events(Events::IN | Events::HUP);
    }
}

impl Consumer {
    fn this_end(&self) -> &EndPoint<RbConsumer<u8>> {
        &self.common.consumer
    }

    fn peer_end(&self) -> &EndPoint<RbProducer<u8>> {
        &self.common.producer
    }

    fn update_pollee(&self) {
        let this_end = self.this_end();
        let peer_end = self.peer_end();

        // Update the event of pollee in a critical region so that pollee
        // always reflects the _true_ state of the underlying ring buffer
        // regardless of any race conditions.
        let event_lock = self.common.lock_event();

        let rb = this_end.ringbuf();
        if rb.is_empty() {
            this_end.pollee().del_events(Events::IN);
        }
        if !rb.is_full() {
            peer_end.pollee().add_events(Events::OUT);
        }
    }
}

impl File for Consumer {
    fn read(&self, buf: &mut [u8]) -> Result<usize> {
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
            let nbytes = rb.pop_slice(buf);
            nbytes
        };

        self.update_pollee();

        if nbytes > 0 {
            Ok(nbytes)
        } else {
            return_errno!(EAGAIN, "try read later");
        }
    }

    // TODO: implement read

    fn poll_by(&self, mask: Events, poller: Option<&mut Poller>) -> Events {
        self.this_end().pollee().poll_by(mask, poller)
    }

    fn as_any(&self) -> &dyn Any {
        self as &dyn Any
    }
}

impl Drop for Consumer {
    fn drop(&mut self) {
        self.peer_end()
            .pollee()
            .add_events(Events::OUT | Events::HUP);
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
        let producer = Async::new(Box::new(producer));
        let consumer = Async::new(Box::new(consumer));

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
        assert!(producer.poll_by(mask, None) == Events::OUT);
        assert!(consumer.poll_by(mask, None) == Events::empty());

        // First write
        producer.write(&buf[..BUF_LEN]);
        assert!(producer.poll_by(mask, None) == Events::OUT);
        assert!(consumer.poll_by(mask, None) == Events::IN);

        // First read, but only half of the avail data
        consumer.read(&mut buf[..BUF_LEN / 2]);
        assert!(producer.poll_by(mask, None) == Events::OUT);
        assert!(consumer.poll_by(mask, None) == Events::IN);

        // Second read, consume the rest of avail data
        consumer.read(&mut buf[..BUF_LEN / 2]);
        assert!(producer.poll_by(mask, None) == Events::OUT);
        assert!(consumer.poll_by(mask, None) == Events::empty());

        // Second and third write, filling up the underlying buffer
        producer.write(&buf[..BUF_LEN]);
        producer.write(&buf[..BUF_LEN]);
        assert!(producer.poll_by(mask, None) == Events::empty());
        assert!(consumer.poll_by(mask, None) == Events::IN);
    }
}
