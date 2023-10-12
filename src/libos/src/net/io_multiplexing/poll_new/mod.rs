use core::hint;
use std::cell::Cell;
use std::sync::Weak;
use std::time::Duration;

use crate::prelude::*;
use crate::socket::util::poller::Poller;
use crate::{fs::IoEvents, net::socket_file::UringSocketType};

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

    let all_uring = files_and_expected_events.iter().all(|f_and_events| {
        if let Some((file, events)) = f_and_events {
            file.as_uring_socket().is_ok()
        } else {
            false
        }
    });

    if all_uring {
        // println!("is uring");
        // The main loop of polling
        let poller = Poller::new();
        loop {
            let mut num_revents = 0;

            for poll_fd in poll_fds {
                // Skip poll_fd if it is not given a fd
                // let fd = match poll_fd.fd() {
                //     Some(fd) => fd,
                //     None => continue,
                // };

                let fd = poll_fd.fd();

                // Poll the file
                let file = current!().file(fd)?;

                let file = file.as_uring_socket().unwrap();

                let need_poller = if num_revents == 0 {
                    Some(&poller)
                } else {
                    None
                };
                let revents = file.poll(poll_fd.events, need_poller);
                if !revents.is_empty() {
                    poll_fd.revents().set(revents);
                    fds[num_revents] = poll_fd.fd;
                    num_revents += 1;
                }
            }

            if num_revents > 0 {
                // println!("fds: {:?}", &fds);
                return Ok(num_revents);
            }

            // Return immediately if specifying a timeout of zero
            if timeout.is_some() && timeout.as_ref().unwrap().is_zero() {
                return Ok(0);
            }

            // Return if the timeout expires.
            if let Err(e) = poller.wait_timeout(None) {
                if e.errno() == ETIMEDOUT {
                    return Ok(0);
                } else {
                    return_errno!(e.errno(), "wait timeout error");
                }
            }
        }
    } else {
        // /// First poll the status
        // let mut count_fds = 0;

        // for (file_and_event, poll_fd) in files_and_expected_events.iter().zip(poll_fds.iter()) {
        //     // Ignore negative poll_fds
        //     if file_and_event.is_none() {
        //         continue;
        //     }
        //     let mask = poll_fd.events;
        //     let file = &file_and_event.as_ref().unwrap().0;
        //     if file.as_uring_socket().is_ok() {
        //         let events = file.poll_new() & mask;
        //         if !events.is_empty() {
        //             poll_fd.revents.set(events);
        //             debug!("poll fd = {:?}, revents = {:?}", poll_fd, events);
        //             count_fds += 1;
        //         }
        //     }
        //     // } else {
        //     //     count_fds = 0;
        //     //     break;
        //     // }
        // }

        // if count_fds > 0 {
        //     // println!("fds: {:?}", count_fds);
        //     return Ok(count_fds);
        // }

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
            let mut count = 0;
            for (file_and_event, poll_fd) in files_and_expected_events.iter().zip(poll_fds.iter()) {
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
                    fds[count] = poll_fd.fd;
                    count += 1;
                }
            }

            // hard code
            // let fds = poll_fds.len();
            // if fds == 10 {
            //     if count >= 4 {
            //         return Ok(count);
            //     }
            // } else {
            //     if count > 0 {
            //         return Ok(count);
            //     }
            // }

            if count > 0 {
                // println!("fds: {:?}", &fds);
                return Ok(count);
            }

            if monitor.is_uring() {
                if let Some(real_timeout) = timeout.as_ref() {
                    if real_timeout.is_zero() {
                        return Ok(0);
                    }
                }
                monitor.uring_wait();

                // let mut spin = 20000;
                // while spin != 0 {
                //     spin -= 1;
                //     hint::spin_loop()
                // }
            } else {
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

            // // Wait for a while to try again later.
            // let ret = monitor.wait_events(timeout);
            // match ret {
            //     Ok(timeout_remain) => {
            //         timeout = timeout_remain;
            //         continue;
            //     }
            //     Err(e) if e.errno() == ETIMEDOUT => {
            //         // Return a count of zero indicating that the time is up
            //         return Ok(0);
            //     }
            //     Err(e) => {
            //         return Err(e);
            //     }
            // }
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
