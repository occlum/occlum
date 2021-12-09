use super::*;
use async_io::event::Poller;
use async_io::event::{Events, Observer};
use async_io::file::StatusFlags;
use async_io::util::channel::{Channel, Consumer, Producer};
use std::any::Any;
use std::sync::{Arc, Weak};

pub type SockEnd = Arc<Inner>;

// TODO: Add SO_SNDBUF and SO_RCVBUF to set/getsockopt to dynamcally change the size.
// This value is got from /proc/sys/net/core/rmem_max and wmem_max that are same on linux.
pub const DEFAULT_BUF_SIZE: usize = 208 * 1024;

/// Constructor of two connected SockEnds
pub fn end_pair(nonblocking: bool) -> Result<(SockEnd, SockEnd)> {
    let status_flag = {
        match nonblocking {
            true => StatusFlags::O_NONBLOCK,
            false => StatusFlags::empty(),
        }
    };

    let (pro_a, con_a) = Channel::with_capacity_and_flags(DEFAULT_BUF_SIZE, status_flag)?.split();
    let (pro_b, con_b) = Channel::with_capacity_and_flags(DEFAULT_BUF_SIZE, status_flag)?.split();

    let mut end_a = Arc::new(Inner {
        addr: RwLock::new(None),
        reader: con_a,
        writer: pro_b,
        peer: Weak::default(),
    });
    let end_b = Arc::new(Inner {
        addr: RwLock::new(None),
        reader: con_b,
        writer: pro_a,
        peer: Arc::downgrade(&end_a),
    });

    unsafe {
        Arc::get_mut_unchecked(&mut end_a).peer = Arc::downgrade(&end_b);
    }

    Ok((end_a, end_b))
}

/// One end of the connected unix socket
pub struct Inner {
    addr: RwLock<Option<TrustedAddr>>,
    reader: Consumer<u8>,
    writer: Producer<u8>,
    peer: Weak<Self>,
}

impl Inner {
    pub fn addr(&self) -> Option<TrustedAddr> {
        self.addr.read().unwrap().clone()
    }

    pub fn set_addr(&self, addr: &TrustedAddr) {
        *self.addr.write().unwrap() = Some(addr.clone());
    }

    pub fn peer_addr(&self) -> Option<TrustedAddr> {
        self.peer.upgrade().map(|end| end.addr().clone()).flatten()
    }

    pub fn set_nonblocking(&self, nonblocking: bool) {
        let status_flag = {
            match nonblocking {
                true => StatusFlags::O_NONBLOCK,
                false => StatusFlags::empty(),
            }
        };
        self.reader.set_status_flags(status_flag);
        self.writer.set_status_flags(status_flag);
    }

    pub fn nonblocking(&self) -> bool {
        let cons_nonblocking = self.reader.status_flags().is_nonblocking();
        let prod_nonblocking = self.reader.status_flags().is_nonblocking();
        assert_eq!(cons_nonblocking, prod_nonblocking);
        cons_nonblocking
    }

    pub async fn readv(&self, bufs: &mut [&mut [u8]]) -> Result<usize> {
        self.reader.readv_async(bufs).await
    }

    pub async fn writev(&self, bufs: &[&[u8]]) -> Result<usize> {
        self.writer.writev_async(bufs).await
    }

    pub fn bytes_to_read(&self) -> usize {
        self.reader.ready_len()
    }

    pub fn shutdown(&self, how: Shutdown) -> Result<()> {
        if !self.is_connected() {
            return_errno!(ENOTCONN, "The socket is not connected.");
        }

        if how.should_shut_read() {
            self.reader.shutdown()
        }

        if how.should_shut_write() {
            self.writer.shutdown()
        }

        Ok(())
    }

    pub fn poll(&self, mask: Events, poller: Option<&mut Poller>) -> Events {
        let mut events = Events::empty();

        let (reader_events, writer_events) = if let Some(poller) = poller {
            let reader_events = self.reader.poll(mask, Some(poller));
            let writer_events = self.writer.poll(mask, Some(poller));
            (reader_events, writer_events)
        } else {
            (self.reader.poll(mask, None), self.writer.poll(mask, None))
        };

        if reader_events.contains(Events::HUP) || self.reader.is_shutdown() {
            events |= Events::RDHUP | Events::IN;
        }
        if writer_events.contains(Events::ERR) || self.writer.is_shutdown() {
            events |= Events::HUP | Events::OUT;
        }

        events |= (reader_events & Events::IN) | (writer_events & Events::OUT);
        events
    }

    pub fn register_observer(&self, observer: Arc<dyn Observer>, mask: Events) {
        self.reader.register_observer(observer.clone(), mask);
        self.writer.register_observer(observer, mask);
    }

    pub fn unregister_observer(&self, observer: &Arc<dyn Observer>) -> Result<Arc<dyn Observer>> {
        self.reader.unregister_observer(observer);
        self.writer.unregister_observer(observer)
    }

    fn is_connected(&self) -> bool {
        self.peer.upgrade().is_some()
    }
}
