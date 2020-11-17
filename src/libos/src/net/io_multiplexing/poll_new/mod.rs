use std::cell::Cell;
use std::sync::Weak;
use std::time::Duration;

use crate::fs::IoEvents;
use crate::prelude::*;

use self::event_monitor::{EventMonitor, EventMonitorBuilder};

mod event_monitor;

// TODO: rename this to do_poll after the old version is removed
pub fn do_poll_new(poll_fds: &[PollFd], mut timeout: Option<&mut Duration>) -> Result<usize> {
    debug!("poll: poll_fds: {:?}, timeout: {:?}", poll_fds, timeout);

    // Always clear the revents fields first
    for poll_fd in poll_fds {
        poll_fd.revents.set(IoEvents::empty());
    }

    // Map poll_fds to FileRef's
    let thread = current!();
    let files: Vec<FileRef> = poll_fds
        .iter()
        .filter_map(|poll_fd| {
            let file = thread.file(poll_fd.fd).ok();

            // Mark an invalid fd by outputting an IoEvents::NVAL event
            if file.is_none() {
                poll_fd.revents.set(IoEvents::NVAL);
            }

            file
        })
        .collect();

    // If there are any invalid fds, then report errors as events
    let num_invalid_fds = poll_fds.len() - files.len();
    if num_invalid_fds > 0 {
        return Ok(num_invalid_fds);
    }

    // Now that all fds are valid, we set up a monitor for the set of files
    let mut monitor = {
        let expected_num_files = files.len();
        let mut builder = EventMonitorBuilder::new(expected_num_files);
        for (file, poll_fd) in files.into_iter().zip(poll_fds.iter()) {
            builder.add_file(file, poll_fd.events);
        }
        builder.build()
    };

    // Poll the set of files until success, timeout, or interruption.
    loop {
        monitor.reset_events();

        // Poll each and every interesting file
        let mut count = 0;
        for (file, poll_fd) in monitor.files().zip(poll_fds.iter()) {
            let mask = poll_fd.events;
            let events = file.poll_new() & mask;
            if !events.is_empty() {
                poll_fd.revents.set(events);
                count += 1;
            }
        }

        if count > 0 {
            return Ok(count);
        }

        // Wait for a while to try again later.
        let ret = monitor.wait_events(timeout);
        match ret {
            Ok(timeout_remain) => {
                timeout = timeout_remain;
                continue;
            }
            Err(e) if e.errno() == ETIMEDOUT => {
                // Return a count of zero indicating that the time is up
                return Ok(0);
            }
            Err(e) => {
                return Err(e);
            }
        }
    }
}

#[derive(Debug)]
pub struct PollFd {
    fd: FileDesc,
    events: IoEvents,
    revents: Cell<IoEvents>,
}

impl PollFd {
    pub fn new(fd: FileDesc, events: IoEvents) -> Self {
        let revents = Cell::new(IoEvents::empty());
        Self {
            fd,
            events,
            revents,
        }
        .add_default_events()
    }

    pub fn from_raw(c_poll_fd: &libc::pollfd) -> Self {
        Self {
            fd: c_poll_fd.fd as FileDesc,
            events: IoEvents::from_raw(c_poll_fd.events as u32),
            revents: Cell::new(IoEvents::from_raw(c_poll_fd.revents as u32)),
        }
        .add_default_events()
    }

    fn add_default_events(mut self) -> Self {
        // Add two default flags to the user-given mask
        self.events |= IoEvents::ERR | IoEvents::HUP;
        self
    }

    pub fn fd(&self) -> FileDesc {
        self.fd
    }

    pub fn events(&self) -> IoEvents {
        self.events
    }

    pub fn revents(&self) -> &Cell<IoEvents> {
        &self.revents
    }

    pub fn to_raw(&self) -> libc::pollfd {
        libc::pollfd {
            fd: self.fd as i32,
            events: self.events.to_raw() as i16,
            revents: self.revents.get().to_raw() as i16,
        }
    }
}
