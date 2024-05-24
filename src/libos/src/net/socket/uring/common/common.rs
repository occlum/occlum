use core::time::Duration;
use std::marker::PhantomData;
use std::sync::atomic::{AtomicBool, Ordering};

use super::Timeout;
use io_uring_callback::IoUring;

use libc::ocall::getsockname as do_getsockname;
use libc::ocall::shutdown as do_shutdown;
use libc::ocall::socket as do_socket;
use libc::ocall::socketpair as do_socketpair;

use crate::events::Pollee;
use crate::fs::{IoEvents, IoNotifier};
use crate::net::socket::uring::runtime::Runtime;
use crate::prelude::*;

/// The common parts of all stream sockets.
pub struct Common<A: Addr + 'static, R: Runtime> {
    host_fd: FileDesc,
    type_: SocketType,
    nonblocking: AtomicBool,
    is_closed: AtomicBool,
    pollee: Pollee,
    inner: Mutex<Inner<A>>,
    timeout: Mutex<Timeout>,
    errno: Mutex<Option<Errno>>,
    io_uring: Arc<IoUring>,
    phantom_data: PhantomData<(A, R)>,
}

impl<A: Addr + 'static, R: Runtime> Common<A, R> {
    pub fn new(type_: SocketType, nonblocking: bool, protocol: Option<i32>) -> Result<Self> {
        let domain_c = A::domain() as libc::c_int;
        let type_c = type_ as libc::c_int;
        let protocol = protocol.unwrap_or(0) as libc::c_int;
        let host_fd = try_libc!(do_socket(domain_c, type_c, protocol)) as FileDesc;
        let nonblocking = AtomicBool::new(nonblocking);
        let is_closed = AtomicBool::new(false);
        let pollee = Pollee::new(IoEvents::empty());
        let inner = Mutex::new(Inner::new());
        let timeout = Mutex::new(Timeout::new());
        let io_uring = R::io_uring();
        let errno = Mutex::new(None);
        Ok(Self {
            host_fd,
            type_,
            nonblocking,
            is_closed,
            pollee,
            inner,
            timeout,
            errno,
            io_uring,
            phantom_data: PhantomData,
        })
    }

    pub fn new_pair(sock_type: SocketType, nonblocking: bool) -> Result<(Self, Self)> {
        return_errno!(EINVAL, "Unix is unsupported");
    }

    pub fn with_host_fd(host_fd: FileDesc, type_: SocketType, nonblocking: bool) -> Self {
        let nonblocking = AtomicBool::new(nonblocking);
        let is_closed = AtomicBool::new(false);
        let pollee = Pollee::new(IoEvents::empty());
        let inner = Mutex::new(Inner::new());
        let timeout = Mutex::new(Timeout::new());
        let io_uring = R::io_uring();
        let errno = Mutex::new(None);
        Self {
            host_fd,
            type_,
            nonblocking,
            is_closed,
            pollee,
            inner,
            timeout,
            errno,
            io_uring,
            phantom_data: PhantomData,
        }
    }

    pub fn io_uring(&self) -> Arc<IoUring> {
        self.io_uring.clone()
    }

    pub fn host_fd(&self) -> FileDesc {
        self.host_fd
    }

    pub fn type_(&self) -> SocketType {
        self.type_
    }

    pub fn nonblocking(&self) -> bool {
        self.nonblocking.load(Ordering::Relaxed)
    }

    pub fn set_nonblocking(&self, is_nonblocking: bool) {
        self.nonblocking.store(is_nonblocking, Ordering::Relaxed)
    }

    pub fn notifier(&self) -> &IoNotifier {
        self.pollee.notifier()
    }

    pub fn send_timeout(&self) -> Option<Duration> {
        self.timeout.lock().sender_timeout()
    }

    pub fn recv_timeout(&self) -> Option<Duration> {
        self.timeout.lock().receiver_timeout()
    }

    pub fn set_send_timeout(&self, timeout: Duration) {
        self.timeout.lock().set_sender(timeout)
    }

    pub fn set_recv_timeout(&self, timeout: Duration) {
        self.timeout.lock().set_receiver(timeout)
    }

    pub fn is_closed(&self) -> bool {
        self.is_closed.load(Ordering::Relaxed)
    }

    pub fn set_closed(&self) {
        self.is_closed.store(true, Ordering::Relaxed)
    }

    pub fn reset_closed(&self) {
        self.is_closed.store(false, Ordering::Relaxed)
    }

    pub fn pollee(&self) -> &Pollee {
        &self.pollee
    }

    #[allow(unused)]
    pub fn addr(&self) -> Option<A> {
        let inner = self.inner.lock();
        inner.addr.clone()
    }

    pub fn set_addr(&self, addr: &A) {
        let mut inner = self.inner.lock();
        inner.addr = Some(addr.clone())
    }

    pub fn get_addr_from_host(&self) -> Result<A> {
        let mut c_addr: libc::sockaddr_storage = unsafe { std::mem::zeroed() };
        let mut c_addr_len = std::mem::size_of::<libc::sockaddr_storage>() as u32;
        try_libc!(do_getsockname(
            self.host_fd as _,
            &mut c_addr as *mut libc::sockaddr_storage as *mut _,
            &mut c_addr_len as *mut _,
        ));
        A::from_c_storage(&c_addr, c_addr_len as _)
    }

    pub fn peer_addr(&self) -> Option<A> {
        let inner = self.inner.lock();
        inner.peer_addr.clone()
    }

    pub fn set_peer_addr(&self, peer_addr: &A) {
        let mut inner = self.inner.lock();
        inner.peer_addr = Some(peer_addr.clone());
    }

    pub fn reset_peer_addr(&self) {
        let mut inner = self.inner.lock();
        inner.peer_addr = None;
    }

    // For getsockopt SO_ERROR command
    pub fn errno(&self) -> Option<Errno> {
        let mut errno_option = self.errno.lock();
        errno_option.take()
    }

    pub fn set_errno(&self, errno: Errno) {
        let mut errno_option = self.errno.lock();
        *errno_option = Some(errno);
    }

    pub fn host_shutdown(&self, how: Shutdown) -> Result<()> {
        trace!("host shutdown: {:?}", how);
        match how {
            Shutdown::Write => {
                try_libc!(do_shutdown(self.host_fd as _, libc::SHUT_WR));
            }
            Shutdown::Read => {
                try_libc!(do_shutdown(self.host_fd as _, libc::SHUT_RD));
            }
            Shutdown::Both => {
                try_libc!(do_shutdown(self.host_fd as _, libc::SHUT_RDWR));
            }
        }
        Ok(())
    }
}

impl<A: Addr + 'static, R: Runtime> std::fmt::Debug for Common<A, R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Common")
            .field("host_fd", &self.host_fd)
            .field("type", &self.type_)
            .field("nonblocking", &self.nonblocking)
            .field("pollee", &self.pollee)
            .field("inner", &self.inner.lock())
            .finish()
    }
}

impl<A: Addr + 'static, R: Runtime> Drop for Common<A, R> {
    fn drop(&mut self) {
        if let Err(e) = super::do_close(self.host_fd) {
            log::error!("do_close failed, host_fd: {}, err: {:?}", self.host_fd, e);
        }

        R::disattach_io_uring(self.host_fd as usize, self.io_uring())
    }
}

#[derive(Debug)]
struct Inner<A: Addr + 'static> {
    addr: Option<A>,
    peer_addr: Option<A>,
}

impl<A: Addr + 'static> Inner<A> {
    pub fn new() -> Self {
        Self {
            addr: None,
            peer_addr: None,
        }
    }
}
