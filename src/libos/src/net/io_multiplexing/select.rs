use super::super::time::timer_slack::TIMERSLACK;
use super::*;

pub fn select(
    nfds: c_int,
    readfds: &mut libc::fd_set,
    writefds: &mut libc::fd_set,
    exceptfds: &mut libc::fd_set,
    timeout: *mut timeval_t,
) -> Result<isize> {
    debug!(
        "read: {} write: {} exception: {}",
        readfds.format(),
        writefds.format(),
        exceptfds.format()
    );

    let mut ready_num = 0;
    let mut pollfds: Vec<PollEvent> = Vec::new();

    for fd in 0..(nfds as FileDesc) {
        let (r, w, e) = (
            readfds.is_set(fd),
            writefds.is_set(fd),
            exceptfds.is_set(fd),
        );
        if !(r || w || e) {
            continue;
        }

        if current!().file(fd).is_err() {
            return_errno!(
                EBADF,
                "An invalid file descriptor was given in one of the sets"
            );
        }

        let mut events = PollEventFlags::empty();
        if r {
            events |= PollEventFlags::POLLIN;
        }
        if w {
            events |= PollEventFlags::POLLOUT;
        }
        if e {
            events |= PollEventFlags::POLLPRI;
        }

        pollfds.push(PollEvent::new(fd, events));
    }

    let mut origin_timeout: timeval_t = if timeout.is_null() {
        Default::default()
    } else {
        unsafe { *timeout }
    };

    let ret = do_poll(&mut pollfds, timeout)?;

    readfds.clear();
    writefds.clear();
    exceptfds.clear();

    if !timeout.is_null() {
        let time_left = unsafe { *(timeout) };
        time_left.validate()?;
        assert!(
            // Note: TIMERSLACK is a single value use maintained by the libOS and will not vary for different threads.
            time_left.as_duration() <= origin_timeout.as_duration() + (*TIMERSLACK).to_duration()
        );
    }

    debug!("returned pollfds are {:?}", pollfds);
    for pollfd in &pollfds {
        let (r_poll, w_poll, e_poll) = convert_to_readable_writable_exceptional(pollfd.revents());
        if r_poll {
            readfds.set(pollfd.fd())?;
            ready_num += 1;
        }
        if w_poll {
            writefds.set(pollfd.fd())?;
            ready_num += 1;
        }
        if e_poll {
            exceptfds.set(pollfd.fd())?;
            ready_num += 1;
        }
    }

    Ok(ready_num)
}

/// Safe methods for `libc::fd_set`
pub trait FdSetExt {
    fn new_empty() -> Self;
    fn unset(&mut self, fd: FileDesc) -> Result<()>;
    fn is_set(&self, fd: FileDesc) -> bool;
    fn set(&mut self, fd: FileDesc) -> Result<()>;
    fn clear(&mut self);
    fn is_empty(&self) -> bool;
    fn as_raw_ptr_mut(&mut self) -> *mut Self;
    fn format(&self) -> String;
}

impl FdSetExt for libc::fd_set {
    fn new_empty() -> Self {
        unsafe { core::mem::zeroed() }
    }

    fn unset(&mut self, fd: FileDesc) -> Result<()> {
        if fd as usize >= libc::FD_SETSIZE {
            return_errno!(EINVAL, "fd exceeds FD_SETSIZE");
        }
        unsafe {
            libc::FD_CLR(fd as c_int, self);
        }
        Ok(())
    }

    fn set(&mut self, fd: FileDesc) -> Result<()> {
        if fd as usize >= libc::FD_SETSIZE {
            return_errno!(EINVAL, "fd exceeds FD_SETSIZE");
        }
        unsafe {
            libc::FD_SET(fd as c_int, self);
        }
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

    fn as_raw_ptr_mut(&mut self) -> *mut Self {
        if self.is_empty() {
            std::ptr::null_mut()
        } else {
            self as *mut libc::fd_set
        }
    }

    fn format(&self) -> String {
        let set = unsafe {
            std::slice::from_raw_parts(self as *const Self as *const u64, libc::FD_SETSIZE / 64)
        };
        format!("libc::fd_set: {:x?}", set)
    }
}

// The correspondence is from man2/select.2.html
fn convert_to_readable_writable_exceptional(events: PollEventFlags) -> (bool, bool, bool) {
    (
        (PollEventFlags::POLLRDNORM
            | PollEventFlags::POLLRDBAND
            | PollEventFlags::POLLIN
            | PollEventFlags::POLLHUP
            | PollEventFlags::POLLERR)
            .intersects(events),
        (PollEventFlags::POLLWRBAND
            | PollEventFlags::POLLWRNORM
            | PollEventFlags::POLLOUT
            | PollEventFlags::POLLERR)
            .intersects(events),
        PollEventFlags::POLLPRI.intersects(events),
    )
}
