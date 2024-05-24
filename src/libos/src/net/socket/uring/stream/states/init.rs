use crate::fs::IoEvents;
use crate::net::socket::uring::common::Common;
use crate::net::socket::uring::runtime::Runtime;
use crate::prelude::*;

/// A stream socket that is in its initial state.
pub struct InitStream<A: Addr + 'static, R: Runtime> {
    common: Arc<Common<A, R>>,
    inner: Mutex<Inner>,
}

struct Inner {
    has_bound: bool,
}

impl<A: Addr + 'static, R: Runtime> InitStream<A, R> {
    pub fn new(nonblocking: bool) -> Result<Arc<Self>> {
        let common = Arc::new(Common::new(SocketType::STREAM, nonblocking, None)?);
        common.pollee().add_events(IoEvents::HUP | IoEvents::OUT);
        let inner = Mutex::new(Inner::new());
        let new_self = Self { common, inner };
        Ok(Arc::new(new_self))
    }

    pub fn new_with_common(common: Arc<Common<A, R>>) -> Result<Arc<Self>> {
        let inner = Mutex::new(Inner {
            has_bound: common.addr().is_some(),
        });
        let new_self = Self { common, inner };
        Ok(Arc::new(new_self))
    }

    pub fn bind(&self, addr: &A) -> Result<()> {
        let mut inner = self.inner.lock();
        if inner.has_bound {
            return_errno!(EINVAL, "the socket is already bound to an address");
        }

        crate::net::socket::uring::common::do_bind(self.common.host_fd(), addr)?;

        inner.has_bound = true;
        self.common.set_addr(addr);
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
            .field("inner", &*self.inner.lock())
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
