use std::time::Duration;

#[derive(Clone, Debug)]
pub struct Timeout {
    sender: Option<Duration>,
    receiver: Option<Duration>,
}

impl Timeout {
    pub fn new() -> Self {
        Self {
            sender: None,
            receiver: None,
        }
    }

    pub fn sender_timeout(&self) -> Option<Duration> {
        self.sender
    }

    pub fn receiver_timeout(&self) -> Option<Duration> {
        self.receiver
    }

    pub fn set_sender(&mut self, timeout: Duration) {
        self.sender = Some(timeout);
    }

    pub fn set_receiver(&mut self, timeout: Duration) {
        self.receiver = Some(timeout);
    }
}
