use super::*;
use alloc::sync::{Arc, Weak};
use fs::channel::{Channel, Consumer, Producer};

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

    end_a.set_nonblocking(nonblocking);
    end_b.set_nonblocking(nonblocking);

    Ok((end_a, end_b))
}

/// One end of the connected unix socket
pub struct Inner {
    addr: RwLock<Option<Addr>>,
    reader: Consumer<u8>,
    writer: Producer<u8>,
    peer: Weak<Self>,
}

impl Inner {
    pub fn addr(&self) -> Option<Addr> {
        self.addr.read().unwrap().clone()
    }

    pub fn set_addr(&self, addr: &Addr) {
        *self.addr.write().unwrap() = Some(addr.clone());
    }

    pub fn peer_addr(&self) -> Option<Addr> {
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

    pub fn shutdown(&self, how: HowToShut) -> Result<()> {
        if !self.is_connected() {
            return_errno!(ENOTCONN, "The socket is not connected.");
        }

        if how.to_shut_read() {
            self.reader.shutdown()
        }

        if how.to_shut_write() {
            self.writer.shutdown()
        }

        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.peer.upgrade().is_some()
    }
}

// TODO: Add SO_SNDBUF and SO_RCVBUF to set/getsockopt to dynamcally change the size.
// This value is got from /proc/sys/net/core/rmem_max and wmem_max that are same on linux.
pub const DEFAULT_BUF_SIZE: usize = 208 * 1024;
