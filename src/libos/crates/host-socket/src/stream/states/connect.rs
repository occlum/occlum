use std::marker::PhantomData;

use io_uring_callback::{Fd, IoHandle};
use sgx_untrusted_alloc::{MaybeUntrusted, UntrustedBox};

use super::Common;
use crate::prelude::*;
use crate::runtime::Runtime;

/// A stream socket that is in its connecting state.
pub struct ConnectingStream<A: Addr + 'static, R: Runtime> {
    common: Arc<Common<A, R>>,
    peer_addr: A,
    req: Mutex<ConnectReq<A>>,
}

struct ConnectReq<A: Addr> {
    io_handle: Option<IoHandle>,
    c_addr: UntrustedBox<libc::sockaddr_storage>,
    c_addr_len: usize,
    errno: Option<Errno>,
    phantom_data: PhantomData<A>,
}

impl<A: Addr + 'static, R: Runtime> ConnectingStream<A, R> {
    pub fn new(peer_addr: &A, common: Arc<Common<A, R>>) -> Result<Arc<Self>> {
        let req = Mutex::new(ConnectReq::new(peer_addr));
        let new_self = Self {
            common,
            peer_addr: peer_addr.clone(),
            req,
        };
        Ok(Arc::new(new_self))
    }

    /// Connect to the peer address.
    pub async fn connect(self: &Arc<Self>) -> Result<()> {
        let pollee = self.common.pollee();
        pollee.reset_events();

        self.initiate_async_connect();

        // Wait for the async connect to complete
        let mut poller = Poller::new();
        loop {
            let events = pollee.poll_by(Events::OUT, Some(&mut poller));
            if !events.is_empty() {
                break;
            }
            poller.wait().await;
        }

        // Finish the async connect
        let req = self.req.lock().unwrap();
        if let Some(e) = req.errno {
            return_errno!(e, "connect failed");
        }
        Ok(())
    }

    fn initiate_async_connect(self: &Arc<Self>) {
        let arc_self = self.clone();
        let callback = move |retval: i32| {
            // Guard against Igao attack
            assert!(retval <= 0);

            if retval == 0 {
                arc_self.common.pollee().add_events(Events::OUT);
            } else {
                // Store the errno
                let mut req = arc_self.req.lock().unwrap();
                let errno = Errno::from(-retval as u32);
                req.errno = Some(errno);
                drop(req);

                arc_self.common.pollee().add_events(Events::ERR);
            }
        };

        let io_uring = self.common.io_uring();
        let mut req = self.req.lock().unwrap();
        let host_fd = self.common.host_fd() as _;
        let c_addr_ptr = req.c_addr.as_ptr();
        let c_addr_len = req.c_addr_len;
        let io_handle = unsafe {
            io_uring.connect(
                Fd(host_fd),
                c_addr_ptr as *const libc::sockaddr,
                c_addr_len as u32,
                callback,
            )
        };
        req.io_handle = Some(io_handle);
    }

    pub fn peer_addr(&self) -> &A {
        &self.peer_addr
    }

    pub fn common(&self) -> &Arc<Common<A, R>> {
        &self.common
    }
}

impl<A: Addr> ConnectReq<A> {
    pub fn new(peer_addr: &A) -> Self {
        let (c_addr_storage, c_addr_len) = peer_addr.to_c_storage();
        Self {
            io_handle: None,
            c_addr: UntrustedBox::new(c_addr_storage),
            c_addr_len,
            errno: None,
            phantom_data: PhantomData,
        }
    }
}

impl<A: Addr, R: Runtime> std::fmt::Debug for ConnectingStream<A, R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConnectingStream")
            .field("common", &self.common)
            .field("peer_addr", &self.peer_addr)
            .field("req", &*self.req.lock().unwrap())
            .finish()
    }
}

impl<A: Addr> std::fmt::Debug for ConnectReq<A> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConnectReq")
            .field("io_handle", &self.io_handle)
            .field("errno", &self.errno)
            .finish()
    }
}
