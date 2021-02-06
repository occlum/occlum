#![cfg_attr(feature = "sgx", no_std)]
#![feature(maybe_uninit_extra)]
#![allow(unused_variables)]
#![allow(dead_code)]

#[cfg(feature = "sgx")]
extern crate sgx_types;
#[cfg(feature = "sgx")]
#[macro_use]
extern crate sgx_tstd as std;
#[cfg(feature = "sgx")]
extern crate sgx_libc as libc;
#[cfg(feature = "sgx")]
extern crate sgx_trts;
#[cfg(feature = "sgx")]
extern crate sgx_untrusted_alloc;

#[cfg(feature = "sgx")]
use std::prelude::v1::*;

mod io;
mod poll;
mod util;

#[cfg(not(feature = "sgx"))]
use std::{
    ops::Deref,
    sync::{Arc, RwLock},
};
#[cfg(feature = "sgx")]
use std::{
    ops::Deref,
    sync::{Arc, SgxRwLock as RwLock},
};

pub use crate::io::IoUringProvider;
use crate::{
    io::{Acceptor, Common, Connector, Receiver, Sender},
    poll::{Events, Poller},
};

/// A IPv4 stream socket with async APIs.
pub struct Socket<P: IoUringProvider> {
    common: Arc<Common<P>>,
    state: RwLock<State<P>>,
}

enum State<P: IoUringProvider> {
    Init,
    Connecting {
        connector: Arc<Connector<P>>,
    },
    Connected {
        sender: Arc<Sender<P>>,
        receiver: Arc<Receiver<P>>,
    },
    Accepting {
        acceptor: Arc<Acceptor<P>>,
    },
}

// Implementation for Socket

pub const DEFAULT_SEND_BUF_SIZE: usize = 16 * 1024;
pub const DEFAULT_RECV_BUF_SIZE: usize = 16 * 1024;

impl<P: IoUringProvider> Socket<P> {
    /// Create a new instance.
    pub fn new() -> Self {
        let common = Arc::new(Common::new());
        let state = RwLock::new(State::Init);
        Self { common, state }
    }

    pub fn bind(&self, addr: &libc::sockaddr_in) -> i32 {
        let state = self.state.read().unwrap();
        match state.deref() {
            State::Init => {
                let fd = self.common.fd();
                let addr_ptr = addr as *const _ as _;
                let addr_len = std::mem::size_of::<libc::sockaddr_in>() as _;
                #[cfg(not(feature = "sgx"))]
                let retval = unsafe { libc::bind(fd, addr_ptr, addr_len) };
                #[cfg(feature = "sgx")]
                let retval = unsafe { libc::ocall::bind(fd, addr_ptr, addr_len) };
                retval
            }
            _ => -libc::EINVAL,
        }
    }

    pub fn listen(&self, backlog: i32) -> i32 {
        let mut state = self.state.write().unwrap();

        // Sanity checks
        match state.deref() {
            State::Init => {
                if backlog < 0 {
                    return -libc::EINVAL;
                }
            }
            _ => {
                return -libc::EINVAL;
            }
        }

        let fd = self.common.fd();
        #[cfg(not(feature = "sgx"))]
        let retval = unsafe { libc::listen(fd, backlog) };
        #[cfg(feature = "sgx")]
        let retval = unsafe { libc::ocall::listen(fd, backlog) };
        if retval < 0 {
            return retval;
        }

        let acceptor = Acceptor::new(backlog as usize, self.common.clone());
        *state = State::Accepting { acceptor };
        0
    }

    pub async fn connect(&self, addr: &libc::sockaddr_in) -> i32 {
        let connector = {
            let mut state = self.state.write().unwrap();
            // Sanity checks
            match state.deref() {
                State::Init => {}
                _ => {
                    return -libc::EINVAL;
                }
            }

            let connector = Arc::new(Connector::new(self.common.clone()));
            *state = State::Connecting {
                connector: connector.clone(),
            };
            connector
        };

        let retval = connector.connect(addr).await;

        // Update the state depending on whether the connect succeeds
        let mut state = self.state.write().unwrap();
        if retval < 0 {
            // Roll back the state to init
            *state = State::Init;
            return retval;
        }
        *state = {
            let sender = Sender::new(self.common.clone(), DEFAULT_SEND_BUF_SIZE);
            let receiver = Receiver::new(self.common.clone(), DEFAULT_RECV_BUF_SIZE);
            State::Connected { sender, receiver }
        };

        // Mark the socket as writable
        self.common.pollee().add(Events::OUT);
        0
    }

    pub async fn write(&self, buf: &[u8]) -> i32 {
        let sender = {
            let state = self.state.read().unwrap();
            // Sanity checks
            match state.deref() {
                State::Connected { sender, .. } => sender.clone(),
                _ => {
                    return -libc::EPIPE;
                }
            }
        };

        let retval = sender.write(buf).await;
        retval
    }

    pub async fn read(&self, buf: &mut [u8]) -> i32 {
        let receiver = {
            let state = self.state.read().unwrap();
            // Sanity checks
            match state.deref() {
                State::Connected { receiver, .. } => receiver.clone(),
                _ => {
                    return -libc::ENOTCONN;
                }
            }
        };

        let retval = receiver.read(buf).await;
        retval
    }

    pub async fn accept(&self, addr: Option<&mut libc::sockaddr_in>) -> Result<Self, i32> {
        let acceptor = {
            let state = self.state.read().unwrap();
            // Sanity checks
            match state.deref() {
                State::Accepting { acceptor } => acceptor.clone(),
                _ => {
                    return Err(-libc::EINVAL);
                }
            }
        };

        let retval = acceptor.accept(addr).await;
        if retval < 0 {
            return Err(retval);
        }

        let common = Arc::new(Common::new_with_fd(retval));
        let state = RwLock::new({
            let sender = Sender::new(common.clone(), DEFAULT_SEND_BUF_SIZE);
            let receiver = Receiver::new(common.clone(), DEFAULT_RECV_BUF_SIZE);
            State::Connected { sender, receiver }
        });
        Ok(Self { common, state })
    }

    pub fn poll(&self, mask: Events, poller: Option<&mut Poller>) -> Events {
        self.common.pollee().poll_by(mask, poller)
    }

    pub fn shutdown(&self, how: i32) -> i32 {
        let (shutdown_sender, shutdown_receiver) = match how {
            libc::SHUT_RD => (false, true),
            libc::SHUT_WR => (true, false),
            libc::SHUT_RDWR => (true, true),
            _ => return -libc::EINVAL,
        };

        let (sender, receiver) = {
            let state = self.state.read().unwrap();
            // Sanity checks
            match state.deref() {
                State::Connected { sender, receiver } => (sender.clone(), receiver.clone()),
                _ => {
                    return -libc::ENOTCONN;
                }
            }
        };

        if shutdown_sender {
            sender.shutdown();
        }
        if shutdown_receiver {
            receiver.shutdown();
        }
        0
    }
}

// TODO: add more unit tests
#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
