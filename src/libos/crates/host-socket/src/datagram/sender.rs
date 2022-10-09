use core::ops::Deref;
use core::sync::atomic::{AtomicBool, Ordering};
use std::ptr::{self};

use io_uring_callback::Fd;
use sgx_untrusted_alloc::{MaybeUntrusted, UntrustedBox};

use crate::common::Common;
use crate::prelude::*;
use crate::runtime::Runtime;

pub struct Sender<A: Addr + 'static, R: Runtime> {
    common: Arc<Common<A, R>>,
    is_shutdown: AtomicBool,
}

impl<A: Addr, R: Runtime> Sender<A, R> {
    pub fn new(common: Arc<Common<A, R>>) -> Self {
        common.pollee().add_events(Events::OUT);
        let is_shutdown = AtomicBool::new(false);
        Self {
            common,
            is_shutdown,
        }
    }

    /// Shutdown udp sender.
    pub fn shutdown(&self) {
        self.is_shutdown.store(true, Ordering::Relaxed)
    }

    /// Reset udp sender shutdown state.
    pub fn reset_shutdown(&self) {
        self.is_shutdown.store(false, Ordering::Relaxed)
    }

    /// Obtain udp sender shutdown state.
    fn is_shutdown(&self) -> bool {
        self.is_shutdown.load(Ordering::Relaxed)
    }

    pub async fn sendmsg(
        &self,
        bufs: &[&[u8]],
        addr: &A,
        flags: SendFlags,
        control: Option<&[u8]>,
    ) -> Result<usize> {
        if self.is_shutdown() {
            return_errno!(Errno::EWOULDBLOCK, "the write has been shutdown")
        }
        let total_len: usize = bufs.iter().map(|buf| buf.len()).sum();
        if total_len > super::MAX_BUF_SIZE {
            return_errno!(EMSGSIZE, "the message is too large")
        }

        let mut send_buf = UntrustedBox::new_uninit_slice(total_len);
        // Copy data from the bufs to the send buffer
        let mut total_copied = 0;
        for buf in bufs {
            send_buf[total_copied..(total_copied + buf.len())].copy_from_slice(buf);
            total_copied += buf.len();
        }

        let send_control_buf = if let Some(msg_control) = control {
            let send_controllen = msg_control.len();
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

        let send_flags = if self.common.nonblocking() || flags.contains(SendFlags::MSG_DONTWAIT) {
            libc::MSG_DONTWAIT as _
        } else {
            0
        };

        // Submit the async send to io_uring
        let complete_fn = move |_retval: i32| {};
        let io_uring = self.common.io_uring();
        let host_fd = Fd(self.common.host_fd() as _);
        trace!(
            "sendmsg hostfd = {:?}, send_flags = {:?}, addr = {:?}",
            host_fd,
            send_flags,
            addr
        );
        let handle = unsafe { io_uring.sendmsg(host_fd, msghdr_ptr, send_flags, complete_fn) };

        let retval = handle.await;
        if retval < 0 {
            // TODO: add PRI event if set SO_SELECT_ERR_QUEUE
            self.common.pollee().add_events(Events::ERR);
            return_errno!(Errno::from(-retval as u32), "sendmsg failed");
        }
        Ok(retval as usize)
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

    let (c_addr_storage, c_addr_len) = if let Some(addr) = addr {
        addr.to_c_storage()
    } else {
        let storage: libc::sockaddr_storage = unsafe { std::mem::zeroed() };
        (storage, 0)
    };
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

#[repr(C)]
struct SendReq {
    msg: libc::msghdr,
    iovec: libc::iovec,
    addr: libc::sockaddr_storage,
}

unsafe impl MaybeUntrusted for SendReq {}
