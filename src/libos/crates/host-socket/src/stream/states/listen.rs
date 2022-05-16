use std::collections::VecDeque;
use std::marker::PhantomData;
use std::mem::size_of;

use io_uring_callback::{Fd, IoHandle};
use sgx_untrusted_alloc::{MaybeUntrusted, UntrustedBox};

use super::ConnectedStream;
use crate::common::{do_close, Common};
use crate::prelude::*;
use crate::runtime::Runtime;

/// A listener stream, ready to accept incoming connections.
pub struct ListenerStream<A: Addr + 'static, R: Runtime> {
    common: Arc<Common<A, R>>,
    inner: Mutex<Inner<A>>,
}

impl<A: Addr + 'static, R: Runtime> ListenerStream<A, R> {
    /// Creates a new listener stream.
    pub fn new(backlog: u32, common: Arc<Common<A, R>>) -> Result<Arc<Self>> {
        let inner = Inner::new(backlog)?;

        Self::do_listen(common.host_fd(), backlog)?;

        common.pollee().reset_events();
        let new_self = Arc::new(Self {
            common,
            inner: Mutex::new(inner),
        });

        // Start async accept requests right as early as possible to improve performance
        {
            let inner = new_self.inner.lock().unwrap();
            new_self.initiate_async_accepts(inner);
        }

        Ok(new_self)
    }

    fn do_listen(host_fd: HostFd, backlog: u32) -> Result<()> {
        let host_fd = host_fd as i32;
        #[cfg(not(feature = "sgx"))]
        let retval = unsafe { libc::listen(host_fd, backlog as _) };
        #[cfg(feature = "sgx")]
        let retval = unsafe { libc::ocall::listen(host_fd, backlog as _) };
        if retval < 0 {
            let errno = Errno::from(-retval as u32);
            return_errno!(errno, "listen failed");
        }
        Ok(())
    }

    pub async fn accept(self: &Arc<Self>, nonblocking: bool) -> Result<Arc<ConnectedStream<A, R>>> {
        let mask = Events::IN;
        // Init the poller only when needed
        let mut poller = None;
        loop {
            // Attempt to accept
            let res = self.try_accept(nonblocking);
            if !res.has_errno(EAGAIN) {
                return res;
            }

            if self.common.nonblocking() {
                return_errno!(EAGAIN, "no connections are present to be accepted");
            }

            // Ensure the poller is initialized
            if poller.is_none() {
                let new_poller = Poller::new();
                self.common.pollee().connect_poller(mask, &new_poller);
                poller = Some(new_poller);
            }
            // Wait for interesting events by polling

            let events = self.common.pollee().poll(mask, None);
            if events.is_empty() {
                poller.as_ref().unwrap().wait().await?;
            }
        }
    }

    pub fn try_accept(self: &Arc<Self>, nonblocking: bool) -> Result<Arc<ConnectedStream<A, R>>> {
        let mut inner = self.inner.lock().unwrap();

        if let Some(errno) = inner.fatal {
            return_errno!(errno, "accept failed");
        }

        let (accepted_fd, accepted_addr) = inner.backlog.pop_completed_req().ok_or_else(|| {
            self.common.pollee().del_events(Events::IN);
            errno!(EAGAIN, "try accept again")
        })?;

        if !inner.backlog.has_completed_reqs() {
            self.common.pollee().del_events(Events::IN);
        }

        self.initiate_async_accepts(inner);

        let common = {
            let common = Arc::new(Common::with_host_fd(accepted_fd, Type::STREAM, nonblocking));
            common.set_peer_addr(&accepted_addr);
            common
        };
        let accepted_stream = ConnectedStream::new(common);
        Ok(accepted_stream)
    }

    fn initiate_async_accepts(self: &Arc<Self>, mut inner: MutexGuard<Inner<A>>) {
        let backlog = &mut inner.backlog;
        while backlog.has_free_entries() {
            backlog.start_new_req(self);
        }
    }

    pub fn common(&self) -> &Arc<Common<A, R>> {
        &self.common
    }

    pub fn cancel_requests(&self) {
        let io_uring = self.common.io_uring();
        let inner = self.inner.lock().unwrap();
        for entry in inner.backlog.entries.iter() {
            if let Entry::Pending { io_handle } = entry {
                unsafe { io_uring.cancel(io_handle) };
            }
        }
    }
}

impl<A: Addr + 'static, R: Runtime> std::fmt::Debug for ListenerStream<A, R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ListenerStream")
            .field("common", &self.common)
            .field("inner", &self.inner.lock().unwrap())
            .finish()
    }
}

/// The mutable, internal state of a listener stream.
struct Inner<A: Addr> {
    backlog: Backlog<A>,
    fatal: Option<Errno>,
}

impl<A: Addr> Inner<A> {
    pub fn new(backlog: u32) -> Result<Self> {
        Ok(Inner {
            backlog: Backlog::with_capacity(backlog as usize)?,
            fatal: None,
        })
    }
}

impl<A: Addr + 'static> std::fmt::Debug for Inner<A> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Inner")
            .field("backlog", &self.backlog)
            .field("fatal", &self.fatal)
            .finish()
    }
}

/// An entry in the backlog.
#[derive(Debug)]
enum Entry {
    /// The entry is free to use.
    Free,
    /// The entry is a pending accept request.
    Pending { io_handle: IoHandle },
    /// The entry is a completed accept request.
    Completed { host_fd: HostFd },
}

impl Default for Entry {
    fn default() -> Self {
        Self::Free
    }
}

/// An async io_uring accept request.
#[derive(Copy, Clone)]
#[repr(C)]
struct AcceptReq {
    c_addr: libc::sockaddr_storage,
    c_addr_len: libc::socklen_t,
}

// Safety. AcceptReq is a C-style struct with C-style fields.
unsafe impl MaybeUntrusted for AcceptReq {}

/// A backlog of incoming connections of a listener stream.
///
/// With backlog, we can start async accept requests, keep track of the pending requests,
/// and maintain the ones that have completed.
struct Backlog<A: Addr> {
    // The entries in the backlog.
    entries: Box<[Entry]>,
    // Arguments of the io_uring requests submitted for the entries in the backlog.
    reqs: UntrustedBox<[AcceptReq]>,
    // The indexes of completed entries.
    completed: VecDeque<usize>,
    // The number of free entries.
    num_free: usize,
    phantom_data: PhantomData<A>,
}

impl<A: Addr> Backlog<A> {
    pub fn with_capacity(capacity: usize) -> Result<Self> {
        if capacity == 0 {
            return_errno!(EINVAL, "capacity cannot be zero");
        }

        let entries = (0..capacity)
            .map(|_| Entry::Free)
            .collect::<Vec<Entry>>()
            .into_boxed_slice();
        let reqs = UntrustedBox::new_uninit_slice(capacity);
        let completed = VecDeque::new();
        let num_free = capacity;
        let new_self = Self {
            entries,
            reqs,
            completed,
            num_free,
            phantom_data: PhantomData,
        };
        Ok(new_self)
    }

    pub fn has_free_entries(&self) -> bool {
        self.num_free > 0
    }

    /// Start a new async accept request, turning a free entry into a pending one.
    pub fn start_new_req<R: Runtime>(&mut self, stream: &Arc<ListenerStream<A, R>>) {
        if stream.common.is_closed() {
            return;
        }
        debug_assert!(self.has_free_entries());

        let entry_idx = self
            .entries
            .iter()
            .position(|entry| matches!(entry, Entry::Free))
            .unwrap();

        let (c_addr_ptr, c_addr_len_ptr) = {
            let accept_req = &mut self.reqs[entry_idx];
            accept_req.c_addr_len = size_of::<libc::sockaddr_storage>() as _;

            let c_addr_ptr = &mut accept_req.c_addr as *mut _ as _;
            let c_addr_len_ptr = &mut accept_req.c_addr_len as _;
            (c_addr_ptr, c_addr_len_ptr)
        };

        let callback = {
            let stream = stream.clone();
            move |retval: i32| {
                let mut inner = stream.inner.lock().unwrap();

                if retval < 0 {
                    // Since most errors that may result from the accept syscall are _not fatal_,
                    // we simply ignore the errno code and try again.
                    //
                    // According to the man page, Linux may report the network errors on an
                    // newly-accepted socket through the accept system call. Thus, we should not
                    // treat the listener socket as "broken" simply because an error is returned
                    // from the accept syscall.
                    //
                    // TODO: throw fatal errors to the upper layer.
                    let errno = Errno::from(-retval as u32);
                    log::error!("Accept error: errno = {}", errno);
                    //inner.fatal = Some(errno);
                    //stream.common.pollee().add_events(Events::ERR);

                    inner.backlog.entries[entry_idx] = Entry::Free;
                    inner.backlog.num_free += 1;
                    // After getting the error from the accept system call, we should not start
                    // the async accept requests again, because this may cause a large number of
                    // io-uring requests to be retried
                    return;
                }

                let host_fd = retval as HostFd;
                inner.backlog.entries[entry_idx] = Entry::Completed { host_fd };
                inner.backlog.completed.push_back(entry_idx);

                stream.common.pollee().add_events(Events::IN);

                stream.initiate_async_accepts(inner);
            }
        };
        let io_uring = stream.common.io_uring();
        let fd = stream.common.host_fd() as i32;
        let flags = 0;
        let io_handle =
            unsafe { io_uring.accept(Fd(fd), c_addr_ptr, c_addr_len_ptr, flags, callback) };
        self.entries[entry_idx] = Entry::Pending { io_handle };
        self.num_free -= 1;
    }

    pub fn has_completed_reqs(&self) -> bool {
        self.completed.len() > 0
    }

    /// Pop a completed async accept request, turing a completed entry into a free one.
    pub fn pop_completed_req(&mut self) -> Option<(HostFd, A)> {
        let completed_idx = self.completed.pop_front()?;
        let accepted_addr = {
            let AcceptReq { c_addr, c_addr_len } = self.reqs[completed_idx].clone();
            A::from_c_storage(&c_addr, c_addr_len as _).unwrap()
        };
        let accepted_fd = {
            let entry = &mut self.entries[completed_idx];
            let accepted_fd = match entry {
                Entry::Completed { host_fd } => *host_fd,
                _ => unreachable!("the entry should have been completed"),
            };
            self.num_free += 1;
            *entry = Entry::Free;
            accepted_fd
        };
        Some((accepted_fd, accepted_addr))
    }
}

impl<A: Addr + 'static> std::fmt::Debug for Backlog<A> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Backlog")
            .field("entries", &self.entries)
            .field("completed", &self.completed)
            .field("num_free", &self.num_free)
            .finish()
    }
}

impl<A: Addr> Drop for Backlog<A> {
    fn drop(&mut self) {
        for entry in self.entries.iter() {
            if let Entry::Completed { host_fd } = entry {
                if let Err(e) = do_close(*host_fd) {
                    log::error!("close fd failed, host_fd: {}, err: {}", host_fd, e);
                }
            }
        }
    }
}
