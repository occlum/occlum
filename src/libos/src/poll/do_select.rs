use std::time::Duration;

use async_io::event::Events;

use super::do_poll::PollFd;
use crate::prelude::*;

pub async fn do_select(
    nfds: FileDesc,
    mut readfds: Option<&mut libc::fd_set>,
    mut writefds: Option<&mut libc::fd_set>,
    mut exceptfds: Option<&mut libc::fd_set>,
    timeout: Option<&mut Duration>,
) -> Result<isize> {
    debug!(
        "do_select: read: {}, write: {}, exception: {}, timeout: {:?}",
        readfds.format(),
        writefds.format(),
        exceptfds.format(),
        timeout,
    );

    if nfds as usize > libc::FD_SETSIZE {
        return_errno!(EINVAL, "nfds is too large");
    }

    // Convert the three fd_set's to an array of PollFd
    let poll_fds = {
        let mut poll_fds = Vec::new();
        for fd in 0..nfds {
            let events = {
                let readable = readfds.is_set(fd);
                let writable = writefds.is_set(fd);
                let except = exceptfds.is_set(fd);
                convert_rwe_to_events(readable, writable, except)
            };

            if events.is_empty() {
                continue;
            }

            let poll_fd = PollFd::new(Some(fd), events);
            poll_fds.push(poll_fd);
        }
        poll_fds
    };
    // Clear up the three input fd_set's, which will be used for output as well
    readfds.clear();
    writefds.clear();
    exceptfds.clear();

    // Do the poll syscall that is equivalent to the select syscall
    let num_ready_fds = super::do_poll::do_poll(&poll_fds, timeout).await?;
    if num_ready_fds == 0 {
        return Ok(0);
    }

    // Convert poll's pollfd results to select's fd_set results
    let mut num_events = 0;
    for poll_fd in &poll_fds {
        let fd = poll_fd.fd().unwrap();
        let revents = poll_fd.revents().get();
        let (readable, writable, exception) = convert_events_to_rwe(&revents);
        if readable {
            readfds.set(fd)?;
            num_events += 1;
        }
        if writable {
            writefds.set(fd)?;
            num_events += 1;
        }
        if exception {
            exceptfds.set(fd)?;
            num_events += 1;
        }
    }
    Ok(num_events)
}

// Convert select's rwe input to poll's IoEvents input according to Linux's
// behavior.
fn convert_rwe_to_events(readable: bool, writable: bool, except: bool) -> Events {
    let mut events = Events::empty();
    if readable {
        events |= Events::IN;
    }
    if writable {
        events |= Events::OUT;
    }
    if except {
        events |= Events::PRI;
    }
    events
}

// Convert poll's IoEvents results to select's rwe results according to Linux's
// behavior.
fn convert_events_to_rwe(events: &Events) -> (bool, bool, bool) {
    let readable = events.intersects(Events::IN | Events::HUP | Events::ERR);
    let writable = events.intersects(Events::OUT | Events::ERR);
    let exception = events.contains(Events::PRI);
    (readable, writable, exception)
}

/// Safe methods for `libc::fd_set`
pub trait FdSetExt {
    fn unset(&mut self, fd: FileDesc) -> Result<()>;
    fn set(&mut self, fd: FileDesc) -> Result<()>;
    fn clear(&mut self);
    fn is_set(&self, fd: FileDesc) -> bool;
    fn is_empty(&self) -> bool;
    fn format(&self) -> String;
}

impl FdSetExt for libc::fd_set {
    fn unset(&mut self, fd: FileDesc) -> Result<()> {
        if fd as usize >= libc::FD_SETSIZE {
            return_errno!(EINVAL, "fd exceeds FD_SETSIZE");
        }
        unsafe { libc::FD_CLR(fd as c_int, self) };
        Ok(())
    }

    fn set(&mut self, fd: FileDesc) -> Result<()> {
        if fd as usize >= libc::FD_SETSIZE {
            return_errno!(EINVAL, "fd exceeds FD_SETSIZE");
        }
        unsafe { libc::FD_SET(fd as c_int, self) };
        Ok(())
    }

    fn clear(&mut self) {
        unsafe {
            libc::FD_ZERO(self);
        }
    }

    fn is_set(&self, fd: FileDesc) -> bool {
        if fd as usize >= libc::FD_SETSIZE {
            return false;
        }
        unsafe { libc::FD_ISSET(fd as c_int, self as *const Self as *mut Self) }
    }

    fn is_empty(&self) -> bool {
        let set = unsafe {
            std::slice::from_raw_parts(self as *const Self as *const u64, libc::FD_SETSIZE / 64)
        };
        set.iter().all(|&x| x == 0)
    }

    fn format(&self) -> String {
        let set = unsafe {
            std::slice::from_raw_parts(self as *const Self as *const u64, libc::FD_SETSIZE / 64)
        };
        format!("libc::fd_set: {:x?}", set)
    }
}

trait FdSetOptionExt {
    fn format(&self) -> String;
    fn is_set(&self, fd: FileDesc) -> bool;
    fn set(&mut self, fd: FileDesc) -> Result<()>;
    fn clear(&mut self);
}

impl FdSetOptionExt for Option<&mut libc::fd_set> {
    fn format(&self) -> String {
        if let Some(inner) = self {
            inner.format()
        } else {
            "(empty)".to_string()
        }
    }

    fn is_set(&self, fd: FileDesc) -> bool {
        if let Some(inner) = self {
            inner.is_set(fd)
        } else {
            false
        }
    }

    fn set(&mut self, fd: FileDesc) -> Result<()> {
        if let Some(inner) = self {
            inner.set(fd)?;
        }
        Ok(())
    }

    fn clear(&mut self) {
        if let Some(inner) = self {
            inner.clear();
        }
    }
}
