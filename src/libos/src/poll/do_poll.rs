use std::cell::Cell;
use std::time::Duration;

use async_io::event::{Events, Pollee, Poller};

use crate::prelude::*;

pub async fn do_poll(poll_fds: &[PollFd], mut timeout: Option<&mut Duration>) -> Result<usize> {
    debug!("poll: poll_fds: {:?}, timeout: {:?}", poll_fds, timeout);

    // Even if there are actually no fds in poll_fds, but timeout is set to negative, poll should
    // wait forever.

    // Always clear the revents fields first
    for poll_fd in poll_fds {
        poll_fd.revents.set(Events::empty());
    }

    // The main loop of polling
    let poller = Poller::new();
    loop {
        let mut num_revents = 0;

        for poll_fd in poll_fds {
            // Skip poll_fd if it is not given a fd
            let fd = match poll_fd.fd() {
                Some(fd) => fd,
                None => continue,
            };

            // Poll the file
            let file = current!().file(fd)?;
            let need_poller = if num_revents == 0 {
                Some(&poller)
            } else {
                None
            };
            let revents = file.poll(poll_fd.events, need_poller);
            if !revents.is_empty() {
                poll_fd.revents().set(revents);
                num_revents += 1;
            }
        }

        if num_revents > 0 {
            return Ok(num_revents);
        }

        // Return immediately if specifying a timeout of zero
        if timeout.is_some() && timeout.as_ref().unwrap().is_zero() {
            return Ok(0);
        }

        // Return if the timeout expires.
        if let Err(e) = poller.wait_timeout(timeout.as_mut()).await {
            if e.errno() == ETIMEDOUT {
                return Ok(0);
            } else {
                return_errno!(e.errno(), "wait timeout error");
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PollFd {
    fd: Option<FileDesc>,
    events: Events,
    revents: Cell<Events>,
}

impl<T: Deref<Target = libc::pollfd>> From<T> for PollFd {
    fn from(c_poll_fd: T) -> Self {
        let fd = if c_poll_fd.fd >= 0 {
            Some(c_poll_fd.fd as FileDesc)
        } else {
            None
        };
        let events = Events::from_bits_truncate(c_poll_fd.events as u32);
        let revents = Cell::new(Events::from_bits_truncate(c_poll_fd.revents as u32));
        Self {
            fd,
            events,
            revents,
        }
    }
}

impl Into<libc::pollfd> for &PollFd {
    fn into(self) -> libc::pollfd {
        libc::pollfd {
            fd: if let Some(fd) = self.fd {
                fd as i32
            } else {
                -1
            },
            events: self.events.bits() as i16,
            revents: self.revents.get().bits() as i16,
        }
    }
}

impl PollFd {
    pub fn new(fd: Option<FileDesc>, events: Events) -> Self {
        let revents = Cell::new(Events::empty());
        Self {
            fd,
            events,
            revents,
        }
    }
    /*
        fn add_default_events(mut self) -> Self {
            // Add two default flags to the user-given mask
            self.events |= Events::ERR | Events::HUP;
            self
        }
    */
    pub fn fd(&self) -> Option<FileDesc> {
        self.fd
    }

    pub fn events(&self) -> Events {
        self.events
    }

    pub fn revents(&self) -> &Cell<Events> {
        &self.revents
    }
}
