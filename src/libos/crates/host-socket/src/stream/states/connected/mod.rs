use async_io::socket::Addr;

use self::recv::Receiver;
use self::send::Sender;
use super::Common;
use crate::prelude::*;
use crate::runtime::Runtime;

mod recv;
mod send;

pub const SEND_BUF_SIZE: usize = 32 * 1024;
pub const RECV_BUF_SIZE: usize = 32 * 1024;

pub struct ConnectedStream<A: Addr + 'static, R: Runtime> {
    common: Arc<Common<A, R>>,
    sender: Sender,
    receiver: Receiver,
}

impl<A: Addr + 'static, R: Runtime> ConnectedStream<A, R> {
    pub fn new(common: Arc<Common<A, R>>) -> Arc<Self> {
        let sender = Sender::new();
        let receiver = Receiver::new();
        let new_self = Self {
            common,
            sender,
            receiver,
        };
        Arc::new(new_self)
    }

    // TODO: implement other methods

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
