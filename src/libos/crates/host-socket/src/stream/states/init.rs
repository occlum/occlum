use async_io::poll::Pollee;
use async_io::socket::Addr;

use super::Common;
use crate::prelude::*;
use crate::runtime::Runtime;

/// A stream socket that is in its initial state.
pub struct InitStream<A: Addr + 'static, R: Runtime> {
    common: Arc<Common<A, R>>,
    inner: Mutex<Inner>,
}

struct Inner {
    has_bound: bool,
}

impl<A: Addr + 'static, R: Runtime> InitStream<A, R> {
    pub fn new() -> Result<Arc<Self>> {
        let new_self = Self {
            common: Arc::new(Common::new()),
            inner: Mutex::new(Inner::new()),
        };
        Ok(Arc::new(new_self))
    }

    pub fn bind(&self, addr: &A) -> Result<()> {
        let mut inner = self.inner.lock().unwrap();
        if inner.has_bound {
            return_errno!(EINVAL, "the socket is already bound to an address");
        }

        Self::do_bind(self.common.host_fd(), addr)?;

        inner.has_bound = true;
        self.common.set_addr(addr);
        Ok(())
    }

    fn do_bind(host_fd: HostFd, addr: &A) -> Result<()> {
        let fd = host_fd as i32;
        let (c_addr_storage, c_addr_len) = addr.to_c_storage();
        let c_addr_ptr = &c_addr_storage as *const _ as _;
        let c_addr_len = c_addr_len as u32;
        #[cfg(not(feature = "sgx"))]
        let retval = unsafe { libc::bind(fd, c_addr_ptr, c_addr_len) };
        #[cfg(feature = "sgx")]
        let retval = unsafe { libc::ocall::bind(fd, c_addr_ptr, c_addr_len) };
        if retval < 0 {
            let errno = Errno::from(-retval as u32);
            return_errno!(errno, "listen failed");
        }
        Ok(())
    }

    pub fn common(&self) -> &Arc<Common<A, R>> {
        &self.common
    }
}

impl<A: Addr + 'static, R: Runtime> std::fmt::Debug for InitStream<A, R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InitStream")
            .field("common", &self.common)
            .field("inner", &*self.inner.lock().unwrap())
            .finish()
    }
}

impl Inner {
    pub fn new() -> Self {
        Self { has_bound: false }
    }
}

impl std::fmt::Debug for Inner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Inner")
            .field("has_bound", &self.has_bound)
            .finish()
    }
}
