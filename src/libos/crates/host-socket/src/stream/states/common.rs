use std::marker::PhantomData;

use io_uring_callback::IoUring;
cfg_if::cfg_if! {
    if #[cfg(feature = "sgx")] {
        use libc::ocall::socket as do_socket;
        use libc::ocall::getsockname as do_getsockname;
    } else {
        use libc::socket as do_socket;
        use libc::getsockname as do_getsockname;
    }
}

use crate::prelude::*;
use crate::runtime::Runtime;

/// The common parts of all stream sockets.
pub struct Common<A: Addr + 'static, R: Runtime> {
    host_fd: HostFd,
    pollee: Pollee,
    inner: Mutex<Inner<A>>,
    phantom_data: PhantomData<(A, R)>,
}

impl<A: Addr + 'static, R: Runtime> Common<A, R> {
    pub fn new() -> Self {
        let domain_c = A::domain() as libc::c_int;
        let host_fd = {
            let retval = unsafe { do_socket(domain_c, libc::SOCK_STREAM, 0) };
            assert!(retval >= 0);
            retval as HostFd
        };
        let pollee = Pollee::new(Events::empty());
        let inner = Mutex::new(Inner::new());
        Self {
            host_fd,
            pollee,
            inner,
            phantom_data: PhantomData,
        }
    }

    pub fn with_host_fd(host_fd: HostFd) -> Self {
        let pollee = Pollee::new(Events::empty());
        let inner = Mutex::new(Inner::new());
        Self {
            host_fd,
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
        let retval = unsafe {
            do_getsockname(
                self.host_fd as _,
                &mut c_addr as *mut libc::sockaddr_storage as *mut _,
                &mut c_addr_len as *mut _,
            )
        };
        if retval != 0 {
            cfg_if::cfg_if! {
                if #[cfg(feature = "sgx")] {
                    let errno = libc::errno();
                } else {
                    let errno = unsafe { libc::__errno_location() };
                }
            }
            return_errno!(Errno::from(errno as u32), "libc do_getsockname error");
        }

        A::from_c_storage(&c_addr, c_addr_len as _)
    }

    pub fn peer_addr(&self) -> Option<A> {
        let inner = self.inner.lock().unwrap();
        inner.peer_addr.clone()
    }

    pub fn set_peer_addr(&self, peer_addr: &A) {
        let mut inner = self.inner.lock().unwrap();
        inner.peer_addr = Some(peer_addr.clone())
    }
}

impl<A: Addr + 'static, R: Runtime> std::fmt::Debug for Common<A, R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Common")
            .field("host_fd", &self.host_fd)
            .field("pollee", &self.pollee)
            .field("inner", &self.inner.lock().unwrap())
            .finish()
    }
}

impl<A: Addr + 'static, R: Runtime> Drop for Common<A, R> {
    fn drop(&mut self) {
        // TODO: close host_fd
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
