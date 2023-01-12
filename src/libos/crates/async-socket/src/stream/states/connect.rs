use std::marker::PhantomData;
use std::sync::atomic::{AtomicBool, Ordering};

use io_uring_callback::{Fd, IoHandle};
use sgx_untrusted_alloc::UntrustedBox;

use crate::common::Common;
use crate::prelude::*;
use crate::runtime::Runtime;

/// A stream socket that is in its connecting state.
pub struct ConnectingStream<A: Addr + 'static, R: Runtime> {
    common: Arc<Common<A, R>>,
    peer_addr: A,
    req: Mutex<ConnectReq<A>>,
    connected: AtomicBool, // Mainly use for nonblocking socket to update status asynchronously
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
            connected: AtomicBool::new(false),
        };
        Ok(Arc::new(new_self))
    }

    /// Connect to the peer address.
    pub async fn connect(self: &Arc<Self>) -> Result<()> {
        let pollee = self.common.pollee();
        pollee.reset_events();

        self.initiate_async_connect();

        if self.common.nonblocking() {
            return_errno!(EINPROGRESS, "non-blocking connect request in progress");
        }

        // Wait for the async connect to complete
        let mask = Events::OUT;
        let poller = Poller::new();
        pollee.connect_poller(mask, &poller);
        let mut timeout = self.common.send_timeout();
        loop {
            let events = pollee.poll(mask, None);
            if !events.is_empty() {
                break;
            }
            let ret = poller.wait_timeout(timeout.as_mut()).await;
            if let Err(e) = ret {
                let errno = e.errno();
                warn!("connect wait errno = {:?}", errno);
                match errno {
                    ETIMEDOUT => {
                        // Cancel connect request if timeout. No need to wait for cancel to complete.
                        self.cancel_connect_request(false).await;
                        // This error code is same as the connect timeout error code on Linux
                        return_errno!(EINPROGRESS, "timeout reached")
                    }
                    _ => {
                        return_errno!(e.errno(), "wait error")
                    }
                }
            }
        }

        // Finish the async connect
        let req = self.req.lock().unwrap();
        if let Some(e) = req.errno {
            return_errno!(e, "connect failed");
        }
        Ok(())
    }

    fn initiate_async_connect(self: &Arc<Self>) {
        let io_uring = self.common.io_uring();
        let mut req = self.req.lock().unwrap();
        // Skip if there is pending request
        if req.io_handle.is_some() {
            return;
        }

        let arc_self = self.clone();
        let callback = move |retval: i32| {
            // Guard against Igao attack
            assert!(retval <= 0);

            let mut req = arc_self.req.lock().unwrap();
            // Release the handle to the async connect
            req.io_handle.take();

            if retval == 0 {
                arc_self.connected.store(true, Ordering::Relaxed);
                arc_self.common.pollee().add_events(Events::OUT);
            } else {
                // Store the errno
                let errno = Errno::from(-retval as u32);
                req.errno = Some(errno);
                drop(req);

                arc_self.connected.store(false, Ordering::Relaxed);
                arc_self.common.pollee().add_events(Events::ERR);
            }
        };

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

    pub async fn cancel_connect_request(&self, need_wait: bool) {
        {
            let io_uring = self.common.io_uring();
            let req = self.req.lock().unwrap();
            if let Some(io_handle) = &req.io_handle {
                unsafe { io_uring.cancel(io_handle) };
            } else {
                return;
            }
        }

        // Wait for the cancel to complete if needed
        if !need_wait {
            return;
        }

        let poller = Poller::new();
        let mask = Events::ERR | Events::IN;
        self.common.pollee().connect_poller(mask, &poller);

        loop {
            let pending_request_exist = {
                let req = self.req.lock().unwrap();
                req.io_handle.is_some()
            };

            if pending_request_exist {
                let mut timeout = Some(Duration::from_secs(10));
                let ret = poller.wait_timeout(timeout.as_mut()).await;
                if let Err(e) = ret {
                    warn!("wait cancel connect request error = {:?}", e.errno());
                    continue;
                }
            } else {
                break;
            }
        }
    }

    #[allow(dead_code)]
    pub fn peer_addr(&self) -> &A {
        &self.peer_addr
    }

    pub fn common(&self) -> &Arc<Common<A, R>> {
        &self.common
    }

    // This can be used in connecting state to check non-blocking connect status.
    pub fn check_connection(&self) -> bool {
        // It is fine whether the load happens before or after the store operation
        self.connected.load(Ordering::Relaxed)
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
            .field("connected", &self.connected)
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
