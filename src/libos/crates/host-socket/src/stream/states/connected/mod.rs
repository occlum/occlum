use self::recv::Receiver;
use self::send::Sender;
use crate::common::Common;
use crate::prelude::*;
use crate::runtime::Runtime;

mod recv;
mod send;

pub const SEND_BUF_SIZE: usize = 128 * 1024;
pub const RECV_BUF_SIZE: usize = 128 * 1024;

pub struct ConnectedStream<A: Addr + 'static, R: Runtime> {
    common: Arc<Common<A, R>>,
    sender: Sender,
    receiver: Receiver,
}

impl<A: Addr + 'static, R: Runtime> ConnectedStream<A, R> {
    pub fn new(common: Arc<Common<A, R>>) -> Arc<Self> {
        common.pollee().reset_events();
        common.pollee().add_events(Events::OUT);

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
        if how.should_shut_read() {
            // Ignore the pending buffer and on-going request will return 0.
            self.common.host_shutdown(Shutdown::Read)?;
            self.receiver.shutdown();
            self.common.pollee().add_events(Events::IN);
        }
        if how.should_shut_write() {
            // Don't call host_shutdown until the content in the pending buffer is sent.
            if self.sender.is_empty() {
                self.common.host_shutdown(Shutdown::Write)?;
            }
            self.sender.shutdown();
            self.common.pollee().add_events(Events::OUT);
        }

        if how == Shutdown::Both {
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
