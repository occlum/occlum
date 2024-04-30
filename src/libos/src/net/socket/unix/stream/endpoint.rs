use super::*;
use events::{Event, EventFilter, Notifier, Observer};
use fs::channel::{Channel, Consumer, Producer};
use fs::{IoEvents, IoNotifier};
use std::any::Any;
use std::sync::{Arc, Weak};

pub type Endpoint = Arc<Inner>;

/// Constructor of two connected Endpoints
pub fn end_pair(nonblocking: bool) -> Result<(Endpoint, Endpoint)> {
    let (pro_a, con_a) = Channel::new(DEFAULT_BUF_SIZE)?.split();
    let (pro_b, con_b) = Channel::new(DEFAULT_BUF_SIZE)?.split();

    let mut end_a = Arc::new(Inner {
        addr: RwLock::new(None),
        reader: con_a,
        writer: pro_b,
        peer: Weak::default(),
        ancillary: RwLock::new(None),
    });
    let end_b = Arc::new(Inner {
        addr: RwLock::new(None),
        reader: con_b,
        writer: pro_a,
        peer: Arc::downgrade(&end_a),
        ancillary: RwLock::new(None),
    });

    unsafe {
        Arc::get_mut_unchecked(&mut end_a).peer = Arc::downgrade(&end_b);
    }

    end_a.set_nonblocking(nonblocking);
    end_b.set_nonblocking(nonblocking);

    Ok((end_a, end_b))
}

/// One end of the connected unix socket
pub struct Inner {
    addr: RwLock<Option<UnixAddr>>,
    reader: Consumer<u8>,
    writer: Producer<u8>,
    peer: Weak<Self>,
    ancillary: RwLock<Option<Ancillary>>,
}

impl Inner {
    pub fn addr(&self) -> Option<UnixAddr> {
        self.addr.read().unwrap().clone()
    }

    pub fn set_addr(&self, addr: &UnixAddr) {
        *self.addr.write().unwrap() = Some(addr.clone());
    }

    pub fn peer_addr(&self) -> Option<UnixAddr> {
        self.peer.upgrade().map(|end| end.addr().clone()).flatten()
    }

    pub fn set_nonblocking(&self, nonblocking: bool) {
        self.reader.set_nonblocking(nonblocking);
        self.writer.set_nonblocking(nonblocking);
    }

    pub fn nonblocking(&self) -> bool {
        let cons_nonblocking = self.reader.is_nonblocking();
        let prod_nonblocking = self.writer.is_nonblocking();
        assert_eq!(cons_nonblocking, prod_nonblocking);
        cons_nonblocking
    }
    pub fn read(&self, buf: &mut [u8]) -> Result<usize> {
        self.reader.pop_slice(buf)
    }

    pub fn write(&self, buf: &[u8]) -> Result<usize> {
        self.writer.push_slice(buf)
    }

    pub fn readv(&self, bufs: &mut [&mut [u8]]) -> Result<usize> {
        self.reader.pop_slices(bufs)
    }

    pub fn writev(&self, bufs: &[&[u8]]) -> Result<usize> {
        self.writer.push_slices(bufs)
    }

    pub fn bytes_to_read(&self) -> usize {
        self.reader.items_to_consume()
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

    pub fn poll(&self) -> IoEvents {
        let mut events = IoEvents::empty();
        let reader_events = self.reader.poll();
        let writer_events = self.writer.poll();

        if reader_events.contains(IoEvents::HUP) || self.reader.is_self_shutdown() {
            events |= IoEvents::RDHUP | IoEvents::IN;
            if writer_events.contains(IoEvents::ERR) || self.writer.is_self_shutdown() {
                events |= IoEvents::HUP | IoEvents::OUT;
            }
        }

        events |= (reader_events & IoEvents::IN) | (writer_events & IoEvents::OUT);
        events
    }

    pub fn ancillary(&self) -> Option<Ancillary> {
        self.ancillary.read().unwrap().clone()
    }

    pub fn set_ancillary(&self, ancillary: Ancillary) {
        self.ancillary.write().unwrap().insert(ancillary);
    }

    pub fn peer_ancillary(&self) -> Option<Ancillary> {
        self.peer.upgrade().map(|end| end.ancillary()).flatten()
    }

    pub(self) fn register_relay_notifier(&self, observer: &Arc<RelayNotifier>) {
        self.reader.notifier().register(
            Arc::downgrade(observer) as Weak<dyn Observer<_>>,
            None,
            None,
        );

        self.writer.notifier().register(
            Arc::downgrade(observer) as Weak<dyn Observer<_>>,
            None,
            None,
        );
    }

    fn is_connected(&self) -> bool {
        self.peer.upgrade().is_some()
    }
}

/// Ancillary data of connected unix socket's sent/received control message.
#[derive(Clone, Debug)]
pub struct Ancillary {
    pub(super) tid: pid_t, // currently store tid to locate file table
}

impl Ancillary {
    pub fn tid(&self) -> pid_t {
        self.tid
    }
}

// TODO: Add SO_SNDBUF and SO_RCVBUF to set/getsockopt to dynamcally change the size.
// This value is got from /proc/sys/net/core/rmem_max and wmem_max that are same on linux.
pub const DEFAULT_BUF_SIZE: usize = 208 * 1024;

/// An observer used to observe both reader and writer of the endpoint. It also contains a
/// notifier that relays the notification of the endpoint.
pub(super) struct RelayNotifier {
    notifier: IoNotifier,
    endpoint: SgxMutex<Option<Endpoint>>,
}

impl RelayNotifier {
    pub fn new() -> Self {
        let notifier = IoNotifier::new();
        let endpoint = SgxMutex::new(None);
        Self { notifier, endpoint }
    }

    pub fn notifier(&self) -> &IoNotifier {
        &self.notifier
    }

    pub fn observe_endpoint(self: &Arc<Self>, endpoint: &Endpoint) {
        endpoint.register_relay_notifier(self);
        *self.endpoint.lock().unwrap() = Some(endpoint.clone());
    }
}

impl Observer<IoEvents> for RelayNotifier {
    fn on_event(&self, event: &IoEvents, _metadata: &Option<Weak<dyn Any + Send + Sync>>) {
        let endpoint = self.endpoint.lock().unwrap();
        // Only endpoint can broadcast events

        let mut event = event.clone();
        // The event of the channel should not be broadcasted directly to socket.
        // The event transformation should be consistant with poll.
        if event.contains(IoEvents::HUP) {
            event -= IoEvents::HUP;
            event |= IoEvents::RDHUP;
        }

        if event.contains(IoEvents::ERR) {
            event -= IoEvents::ERR;
            event |= IoEvents::HUP;
        }

        // A notifier can only have events after observe_endpoint
        self.notifier()
            .broadcast(&(endpoint.as_ref().unwrap().poll() & event));
    }
}
