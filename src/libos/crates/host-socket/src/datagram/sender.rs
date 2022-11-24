use std::mem::MaybeUninit;
use std::ptr::{self};

use io_uring_callback::{Fd, IoHandle};
use sgx_untrusted_alloc::{MaybeUntrusted, UntrustedBox};

use crate::common::Common;
use crate::prelude::*;
use crate::runtime::Runtime;

pub struct Sender<A: Addr + 'static, R: Runtime> {
    common: Arc<Common<A, R>>,
    inner: Mutex<Inner>,
}

impl<A: Addr, R: Runtime> Sender<A, R> {
    pub fn new(common: Arc<Common<A, R>>) -> Arc<Self> {
        common.pollee().add_events(Events::OUT);
        let inner = Mutex::new(Inner::new());
        Arc::new(Self { common, inner })
    }

    /// Shutdown udp sender.
    pub fn shutdown(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.is_shutdown = true;
    }

    /// Reset udp sender shutdown state.
    pub fn reset_shutdown(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.is_shutdown = false;
    }

    pub fn cancel_send_requests(&self) {
        let io_uring = self.common.io_uring();
        let inner = self.inner.lock().unwrap();
        if let Some(io_handle) = &inner.io_handle {
            unsafe { io_uring.cancel(io_handle) };
        }
    }

    pub async fn sendmsg(
        self: &Arc<Self>,
        bufs: &[&[u8]],
        addr: &A,
        flags: SendFlags,
        control: Option<&[u8]>,
    ) -> Result<usize> {
        let mut inner = self.inner.lock().unwrap();
        if inner.is_shutdown() {
            return_errno!(Errno::EWOULDBLOCK, "the write has been shutdown")
        }
        let total_len: usize = bufs.iter().map(|buf| buf.len()).sum();
        if total_len > super::MAX_BUF_SIZE {
            return_errno!(EMSGSIZE, "the message is too large")
        }

        // Mark the socket as non-writable since Datagram uses single packet
        self.common.pollee().del_events(Events::OUT);

        let mut send_buf = UntrustedBox::new_uninit_slice(total_len);
        // Copy data from the bufs to the send buffer
        let mut total_copied = 0;
        for buf in bufs {
            send_buf[total_copied..(total_copied + buf.len())].copy_from_slice(buf);
            total_copied += buf.len();
        }

        let send_control_buf = if let Some(msg_control) = control {
            let send_controllen = msg_control.len();
            if send_controllen > super::OPTMEM_MAX {
                return_errno!(EINVAL, "invalid msg control length");
            }
            let mut send_control_buf = UntrustedBox::new_uninit_slice(send_controllen);
            send_control_buf.copy_from_slice(&msg_control[..send_controllen]);
            Some(send_control_buf)
        } else {
            None
        };

        // Generate the async send request
        let mut send_req = UntrustedBox::<SendReq>::new_uninit();
        let send_control_buf = send_control_buf.as_ref().map(|buf| &**buf);
        let msghdr_ptr = new_send_req(&mut send_req, &send_buf, addr, send_control_buf);

        // Handle msg flags
        let send_flags = if self.common.nonblocking() || flags.contains(SendFlags::MSG_DONTWAIT) {
            libc::MSG_DONTWAIT as _
        } else {
            0
        };

        // Need to handle MSG_DONTWAIT and nonblocking().
        self.do_send(&mut inner, msghdr_ptr, send_flags);

        // Release inner lock to avoid lock comptetion in do_send (complete_fn)
        drop(inner);

        // Datagram send timeout
        let mask = Events::OUT;
        let poller = Poller::new();
        self.common.pollee().connect_poller(mask, &poller);

        let events = self.common.pollee().poll(mask, None);
        if events.is_empty() {
            let ret = poller
                .wait_timeout(self.common.send_timeout().as_mut())
                .await;
            if let Err(e) = ret {
                warn!("send wait errno = {:?}", e.errno());
                match e.errno() {
                    ETIMEDOUT => {
                        self.cancel_send_requests();
                        return_errno!(EAGAIN, "timeout reached")
                    }
                    _ => {
                        // May need to handle inner error state and event
                        return_errno!(e.errno(), "wait error")
                    }
                }
            }
        }

        let mut inner = self.inner.lock().unwrap();
        if let Some(errno) = inner.error {
            // Reset error
            inner.error = None;
            self.common.pollee().del_events(Events::ERR);
            return_errno!(errno, "write failed");
        }

        Ok(total_copied)
    }

    fn do_send(
        self: &Arc<Self>,
        inner: &mut MutexGuard<Inner>,
        msghdr_ptr: *mut libc::msghdr,
        flags: u32,
    ) {
        let sender = self.clone();
        // Submit the async send to io_uring
        let complete_fn = move |retval: i32| {
            let mut inner = sender.inner.lock().unwrap();
            trace!("send request complete with retval: {}", retval);

            // Release the handle to the async recv
            inner.io_handle.take();

            if retval < 0 {
                // TODO: add PRI event if set SO_SELECT_ERR_QUEUE
                let errno = Errno::from(-retval as u32);

                inner.error = Some(errno);
                sender.common.pollee().add_events(Events::ERR);
                return;
            }

            // Need to handle normal case
            sender.common.pollee().add_events(Events::OUT);
        };

        // Generate the async recv request
        let io_uring = self.common.io_uring();
        let host_fd = Fd(self.common.host_fd() as _);
        let handle = unsafe { io_uring.sendmsg(host_fd, msghdr_ptr, flags, complete_fn) };
        inner.io_handle.replace(handle);
    }
}

fn new_send_req<A: Addr>(
    req: &mut SendReq,
    buf: &[u8],
    addr: &A,
    msg_control: Option<&[u8]>,
) -> *mut libc::msghdr {
    req.iovec = libc::iovec {
        iov_base: buf.as_ptr() as _,
        iov_len: buf.len(),
    };
    req.msg.msg_iov = &raw mut req.iovec as _;
    req.msg.msg_iovlen = 1;

    let (c_addr_storage, c_addr_len) = addr.to_c_storage();

    req.addr = c_addr_storage;
    req.msg.msg_name = &raw mut req.addr as _;
    req.msg.msg_namelen = c_addr_len as _;

    match msg_control {
        Some(inner_control) => {
            req.msg.msg_control = inner_control.as_ptr() as _;
            req.msg.msg_controllen = inner_control.len() as _;
        }
        None => {
            req.msg.msg_control = ptr::null_mut();
            req.msg.msg_controllen = 0;
        }
    }

    &mut req.msg
}

pub struct Inner {
    io_handle: Option<IoHandle>,
    error: Option<Errno>,
    is_shutdown: bool,
}

unsafe impl Send for Inner {}

impl Inner {
    pub fn new() -> Self {
        Self {
            io_handle: None,
            error: None,
            is_shutdown: false,
        }
    }

    /// Obtain udp sender shutdown state.
    pub fn is_shutdown(&self) -> bool {
        self.is_shutdown
    }
}

#[repr(C)]
struct SendReq {
    msg: libc::msghdr,
    iovec: libc::iovec,
    addr: libc::sockaddr_storage,
}

unsafe impl MaybeUntrusted for SendReq {}
