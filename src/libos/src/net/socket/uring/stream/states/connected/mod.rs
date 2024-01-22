use atomic::Ordering;

use self::recv::Receiver;
use self::send::Sender;
use crate::fs::IoEvents as Events;
use crate::net::socket::sockopt::SockOptName;
use crate::net::socket::uring::common::Common;
use crate::net::socket::uring::runtime::Runtime;
use crate::prelude::*;

mod recv;
mod send;

pub struct ConnectedStream<A: Addr + 'static, R: Runtime> {
    common: Arc<Common<A, R>>,
    sender: Sender,
    receiver: Receiver,
}

impl<A: Addr + 'static, R: Runtime> ConnectedStream<A, R> {
    pub fn new(common: Arc<Common<A, R>>) -> Arc<Self> {
        common.pollee().reset_events();
        common.pollee().add_events(Events::OUT);

        let fd = common.host_fd();

        let sender = Sender::new();
        let receiver = Receiver::new();
        let new_self = Arc::new(Self {
            common,
            sender,
            receiver,
        });

        // Start async recv requests right as early as possible to support poll and
        // improve performance. If we don't start recv requests early, the poll()
        // might block forever when user just invokes poll(Event::In) without read().
        // Once we have recv requests completed, we can have Event::In in the events.
        new_self.initiate_async_recv();

        new_self
    }

    pub fn common(&self) -> &Arc<Common<A, R>> {
        &self.common
    }

    pub fn shutdown(&self, how: Shutdown) -> Result<()> {
        // Do host shutdown
        // For shutdown write, don't call host_shutdown until the content in the pending buffer is sent.
        // For shutdown read, ignore the pending buffer.
        let (shut_write, send_buf_is_empty, shut_read) = (
            how.should_shut_write(),
            self.sender.is_empty(),
            how.should_shut_read(),
        );
        match (shut_write, send_buf_is_empty, shut_read) {
            // As long as send buf is empty, just shutdown.
            (_, true, _) => self.common.host_shutdown(how)?,
            // If not shutdown write, just shutdown.
            (false, _, _) => self.common.host_shutdown(how)?,
            // If shutdown both but the send buf is not empty, only shutdown read.
            (true, false, true) => self.common.host_shutdown(Shutdown::Read)?,
            // If shutdown write but the send buf is not empty, don't do shutdown.
            (true, false, false) => {}
        }

        // Set internal state and trigger events.
        if shut_read {
            self.receiver.shutdown();
            self.common.pollee().add_events(Events::IN);
        }
        if shut_write {
            self.sender.shutdown();
            self.common.pollee().add_events(Events::OUT);
        }

        if shut_read && shut_write {
            self.common.pollee().add_events(Events::HUP);
        }

        Ok(())
    }

    pub fn set_closed(&self) {
        // Mark the sender and receiver to shutdown to prevent submitting new requests.
        self.receiver.shutdown();
        self.sender.shutdown();

        self.common.set_closed();
    }

    // Other methods are implemented in the send and receive modules
}

impl<A: Addr + 'static, R: Runtime> std::fmt::Debug for ConnectedStream<A, R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConnectedStream")
            .field("common", &self.common)
            .field("sender", &self.sender)
            .field("receiver", &self.receiver)
            .finish()
    }
}

fn new_msghdr(iovecs_ptr: *mut libc::iovec, iovecs_len: usize) -> libc::msghdr {
    use std::mem::MaybeUninit;
    // Safety. Setting all fields to zeros is a valid state for msghdr.
    let mut msghdr: libc::msghdr = unsafe { MaybeUninit::zeroed().assume_init() };
    msghdr.msg_iov = iovecs_ptr;
    msghdr.msg_iovlen = iovecs_len as _;
    // We do want to leave all other fields as zeros
    msghdr
}
