use super::*;

#[derive(Debug, Copy, Clone)]
pub enum EpollCtlCmd {
    /// Add a file decriptor to the interface
    Add = 1,
    /// Remove a file decriptor from the interface
    Del = 2,
    /// Change file decriptor epoll_event structre
    Mod = 3,
}

impl TryFrom<i32> for EpollCtlCmd {
    type Error = error::Error;

    fn try_from(op_num: i32) -> Result<Self> {
        match op_num {
            1 => Ok(EpollCtlCmd::Add),
            2 => Ok(EpollCtlCmd::Del),
            3 => Ok(EpollCtlCmd::Mod),
            _ => return_errno!(EINVAL, "invalid operation number"),
        }
    }
}

bitflags! {
    #[derive(Default)]
    pub struct EpollEventFlags: u32 {
        // The available events are got from linux source.
        // This struct contains more flags than linux man page described.
        const EPOLLIN        = 0x0001;
        const EPOLLPRI       = 0x0002;
        const EPOLLOUT       = 0x0004;
        const EPOLLERR       = 0x0008;
        const EPOLLHUP       = 0x0010;
        const EPOLLNVAL      = 0x0020;
        const EPOLLRDNORM    = 0x0040;
        const EPOLLRDBAND    = 0x0080;
        const EPOLLWRNORM    = 0x0100;
        const EPOLLWRBAND    = 0x0200;
        const EPOLLMSG       = 0x0400;
        const EPOLLRDHUP     = 0x2000;
        const EPOLLEXCLUSIVE = (1 << 28);
        const EPOLLWAKEUP    = (1 << 29);
        const EPOLLONESHOT   = (1 << 30);
        const EPOLLET        = (1 << 31);
    }
}

//TODO: Add more mitigations to protect from iago attacks
#[derive(Copy, Clone, Debug, Default)]
pub struct EpollEvent {
    /// Epoll Events
    events: EpollEventFlags,
    /// Libos-agnostic user data variable
    data: uint64_t,
}

impl EpollEvent {
    pub fn new(events: EpollEventFlags, data: uint64_t) -> Self {
        Self { events, data }
    }

    pub fn from_raw(epoll_event: &libc::epoll_event) -> Result<Self> {
        Ok(Self::new(
            EpollEventFlags::from_bits(epoll_event.events)
                .ok_or_else(|| errno!(EINVAL, "invalid flags"))?,
            epoll_event.u64,
        ))
    }

    pub fn to_raw(&self) -> libc::epoll_event {
        libc::epoll_event {
            events: self.events.bits(),
            u64: self.data,
        }
    }
}

#[derive(Debug)]
pub struct EpollFile {
    host_fd: c_int,
}

impl EpollFile {
    /// Creates a new Linux epoll file descriptor
    pub fn new(flags: CreationFlags) -> Result<Self> {
        debug!("create epollfile: flags: {:?}", flags);
        let host_fd = try_libc!(libc::ocall::epoll_create1(flags.bits() as i32));
        Ok(Self { host_fd })
    }

    pub fn control(&self, op: EpollCtlCmd, fd: FileDesc, event: Option<&EpollEvent>) -> Result<()> {
        let host_fd = {
            let fd_ref = process::get_file(fd)?;
            if let Ok(socket) = fd_ref.as_socket() {
                socket.fd()
            } else if let Ok(eventfd) = fd_ref.as_event() {
                eventfd.get_host_fd()
            } else {
                return_errno!(EPERM, "unsupported file type");
            }
        };

        // Notes on deadlock.
        //
        // All locks on fd (if any) will be released at this point. This means
        // we don't have to worry about the potential deadlock caused by
        // locking two files (say, fd and epfd) in an inconsistent order.

        let raw_epevent_ptr: *mut libc::epoll_event = match event {
            Some(epevent) => {
                //TODO: Shoud be const.
                // Cast const to mut to be compatiable with the ocall from rust sdk.
                &mut epevent.to_raw()
            }
            _ => std::ptr::null_mut(),
        };

        try_libc!(libc::ocall::epoll_ctl(
            self.host_fd,
            op as i32,
            host_fd,
            raw_epevent_ptr,
        ));
        Ok(())
    }

    /// Waits for an I/O event on the epoll file.
    ///
    /// Returns the number of file descriptors ready for the requested I/O.
    pub fn wait(&self, events: &mut [EpollEvent], timeout: c_int) -> Result<usize> {
        let mut raw_events: Vec<libc::epoll_event> =
            vec![libc::epoll_event { events: 0, u64: 0 }; events.len()];
        let ret = try_libc!(libc::ocall::epoll_wait(
            self.host_fd,
            raw_events.as_mut_ptr(),
            raw_events.len() as c_int,
            timeout,
        )) as usize;

        assert!(ret <= events.len());
        for i in 0..ret {
            events[i] = EpollEvent::from_raw(&raw_events[i])?;
        }

        Ok(ret)
    }
}

impl Drop for EpollFile {
    fn drop(&mut self) {
        unsafe {
            libc::ocall::close(self.host_fd);
        }
    }
}

impl File for EpollFile {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub trait AsEpollFile {
    fn as_epfile(&self) -> Result<&EpollFile>;
}

impl AsEpollFile for FileRef {
    fn as_epfile(&self) -> Result<&EpollFile> {
        self.as_any()
            .downcast_ref::<EpollFile>()
            .ok_or_else(|| errno!(EBADF, "not an epoll file"))
    }
}
