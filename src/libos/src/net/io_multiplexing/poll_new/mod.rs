use core::hint;
use core::slice::Iter;
use std::cell::Cell;
use std::sync::Weak;
use std::time::Duration;

use crate::net::uring_socket::util::poller::Poller;
use crate::prelude::*;
use crate::{fs::IoEvents, net::uring_socket::UringSocketType};

use self::event_monitor::{EventMonitor, EventMonitorBuilder};

mod event_monitor;

// TODO: rename this to do_poll after the old version is removed
pub fn do_poll_new(poll_fds: &[PollFd], mut timeout: Option<&mut Duration>) -> Result<usize> {
    debug!("poll: poll_fds: {:?}, timeout: {:?}", poll_fds, timeout);

    // Always clear the revents fields first
    for poll_fd in poll_fds {
        poll_fd.revents.set(IoEvents::empty());
    }

    let mut fds = [0; 10];

    // Map valid and non-negative poll_fds to FileRef's
    let thread = current!();
    let mut invalid_fd_count = 0;
    let files_and_expected_events: Vec<Option<(FileRef, IoEvents)>> = poll_fds
        .iter()
        .map(|poll_fd| {
            if (poll_fd.fd as i32) < 0 {
                // If poll_fd is negative, ignore the events.
                return None;
            }

            let file = thread.file(poll_fd.fd).ok();

            // Mark an invalid fd by outputting an IoEvents::NVAL event
            if file.is_none() {
                poll_fd.revents.set(IoEvents::NVAL);
                invalid_fd_count += 1;
                return None;
            }

            Some((file.unwrap(), poll_fd.events))
        })
        .collect();

    // If there are any invalid fds, then report errors as events
    if invalid_fd_count > 0 {
        return Ok(invalid_fd_count);
    }

    debug_assert!(files_and_expected_events.len() == poll_fds.len());

    // Fast poll
    let count = poll_interested_events(files_and_expected_events.iter(), poll_fds.iter());
    if count > 0 {
        return Ok(count);
    }

    // Now that all fds are valid, we set up a monitor for the set of files
    let mut monitor = {
        let expected_num_files = files_and_expected_events.len();
        let mut builder = EventMonitorBuilder::new(expected_num_files);
        for file_and_expect_event in files_and_expected_events.iter() {
            if let Some((file_and_event)) = file_and_expect_event {
                builder.add_file(file_and_event.0.clone(), file_and_event.1);
            }
            // Ignore negative poll_fds
        }
        builder.build()
    };

    // Poll the set of files until success, timeout, or interruption.
    loop {
        monitor.reset_events();

        // Poll each and every interesting file
        let count = poll_interested_events(files_and_expected_events.iter(), poll_fds.iter());
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

fn poll_interested_events(
    files_and_expected_events_iter: Iter<Option<(FileRef, IoEvents)>>,
    poll_fds_iter: Iter<PollFd>,
) -> usize {
    let mut count = 0;
    for (file_and_event, poll_fd) in files_and_expected_events_iter.zip(poll_fds_iter) {
        // Ignore negative poll_fds
        if file_and_event.is_none() {
            continue;
        }
        let mask = poll_fd.events;
        let file = &file_and_event.as_ref().unwrap().0;
        let events = file.poll_new() & mask;
        if !events.is_empty() {
            poll_fd.revents.set(events);
            debug!("poll fd = {:?}, revents = {:?}", poll_fd, events);
            count += 1;
        }
    }

    count
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
