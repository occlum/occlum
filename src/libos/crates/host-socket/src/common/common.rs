use std::marker::PhantomData;
use std::sync::atomic::{AtomicBool, Ordering};

use async_io::socket::UnixAddr;
use io_uring_callback::IoUring;
cfg_if::cfg_if! {
    if #[cfg(feature = "sgx")] {
        use libc::ocall::socket as do_socket;
        use libc::ocall::getsockname as do_getsockname;
        use libc::ocall::socketpair as do_socketpair;
    } else {
        use libc::socket as do_socket;
        use libc::getsockname as do_getsockname;
        use libc::socketpair as do_socketpair;
    }
}

use crate::prelude::*;
use crate::runtime::Runtime;

/// The common parts of all stream sockets.
pub struct Common<A: Addr + 'static, R: Runtime> {
    host_fd: HostFd,
    type_: Type,
    nonblocking: AtomicBool,
    is_closed: AtomicBool,
    pollee: Pollee,
    inner: Mutex<Inner<A>>,
    phantom_data: PhantomData<(A, R)>,
}

impl<A: Addr + 'static, R: Runtime> Common<A, R> {
    pub fn new(type_: Type, nonblocking: bool) -> Result<Self> {
        let domain_c = A::domain() as libc::c_int;
        let type_c = type_ as libc::c_int;
        let host_fd = try_libc!(do_socket(domain_c, type_c, 0)) as HostFd;
        let nonblocking = AtomicBool::new(nonblocking);
        let is_closed = AtomicBool::new(false);
        let pollee = Pollee::new(Events::empty());
        let inner = Mutex::new(Inner::new());
        Ok(Self {
            host_fd,
            type_,
            nonblocking,
            is_closed,
            pollee,
            inner,
            phantom_data: PhantomData,
        })
    }

    pub fn new_pair(sock_type: Type, nonblocking: bool) -> Result<(Self, Self)> {
        if A::domain() != Domain::Unix {
            return_errno!(EAFNOSUPPORT, "unsupported domain");
        }
        let domain_c = Domain::Unix as libc::c_int;
        let type_c = sock_type as libc::c_int;
        let mut socks = [0; 2];
        try_libc!(do_socketpair(domain_c, type_c, 0, socks.as_mut_ptr()));

        let common1 = Self::with_host_fd(socks[0] as HostFd, sock_type, nonblocking);
        let mut inner1 = common1.inner.lock().unwrap();
        // addr and peer_addr should be UnixAddr::Unnamed
        inner1.addr = Some(A::default());
        inner1.peer_addr = Some(A::default());
        drop(inner1);

        let common2 = Self::with_host_fd(socks[1] as HostFd, sock_type, nonblocking);
        let mut inner2 = common2.inner.lock().unwrap();
        inner2.addr = Some(A::default());
        inner2.peer_addr = Some(A::default());
        drop(inner2);

        Ok((common1, common2))
    }

    pub fn with_host_fd(host_fd: HostFd, type_: Type, nonblocking: bool) -> Self {
        let nonblocking = AtomicBool::new(nonblocking);
        let is_closed = AtomicBool::new(false);
        let pollee = Pollee::new(Events::empty());
        let inner = Mutex::new(Inner::new());
        Self {
            host_fd,
            type_,
            nonblocking,
            is_closed,
            pollee,
            inner,
            phantom_data: PhantomData,
        }
    }

    pub fn io_uring(&self) -> &IoUring {
        R::io_uring()
    }

    pub fn host_fd(&self) -> HostFd {
        self.host_fd
    }

    pub fn type_(&self) -> Type {
        self.type_
    }

    pub fn nonblocking(&self) -> bool {
        self.nonblocking.load(Ordering::Relaxed)
    }

    pub fn set_nonblocking(&self, is_nonblocking: bool) {
        self.nonblocking.store(is_nonblocking, Ordering::Relaxed)
    }

    pub fn is_closed(&self) -> bool {
        self.is_closed.load(Ordering::Relaxed)
    }

    pub fn set_closed(&self) {
        self.is_closed.store(true, Ordering::Relaxed)
    }

    pub fn pollee(&self) -> &Pollee {
        &self.pollee
    }

    pub fn addr(&self) -> Option<A> {
        let inner = self.inner.lock().unwrap();
        inner.addr.clone()
    }

    pub fn set_addr(&self, addr: &A) {
        let mut inner = self.inner.lock().unwrap();
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
        let inner = self.inner.lock().unwrap();
        inner.peer_addr.clone()
    }

    pub fn set_peer_addr(&self, peer_addr: &A) {
        let mut inner = self.inner.lock().unwrap();
        inner.peer_addr = Some(peer_addr.clone());
    }

    pub fn reset_peer_addr(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.peer_addr = None;
    }
}

impl<A: Addr + 'static, R: Runtime> std::fmt::Debug for Common<A, R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Common")
            .field("host_fd", &self.host_fd)
            .field("type", &self.type_)
            .field("nonblocking", &self.nonblocking)
            .field("pollee", &self.pollee)
            .field("inner", &self.inner.lock().unwrap())
            .finish()
    }
}

impl<A: Addr + 'static, R: Runtime> Drop for Common<A, R> {
    fn drop(&mut self) {
        if let Err(e) = super::do_close(self.host_fd) {
            log::error!("do_close failed, host_fd: {}, err: {:?}", self.host_fd, e);
        }

        if A::domain() != Domain::Unix {
            return;
        }
        if let Some(addr) = self.addr() {
            if let Some(unix_addr) = addr.as_any().downcast_ref::<UnixAddr>() {
                if let UnixAddr::Pathname(path) = unix_addr {
                    if let Err(e) = super::do_unlink(path) {
                        log::error!("do_unlink failed, path: {}, err: {:?}", path, e);
                    }
                }
            }
        }
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
