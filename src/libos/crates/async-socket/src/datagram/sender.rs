use std::ptr::{self};

use io_uring_callback::{Fd, IoHandle};
use libc::c_void;
use sgx_untrusted_alloc::{MaybeUntrusted, UntrustedBox};
use std::collections::VecDeque;

use crate::common::Common;
use crate::prelude::*;
use crate::runtime::Runtime;

const SENDMSG_QUEUE_LEN: usize = 16;

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
        inner.is_shutdown = ShutdownStatus::PreShutdown;
    }

    /// Reset udp sender shutdown state.
    pub fn reset_shutdown(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.is_shutdown = ShutdownStatus::Running;
    }

    /// Whether no buffer in sender.
    pub fn is_empty(&self) -> bool {
        let inner = self.inner.lock().unwrap();
        inner.msg_queue.is_empty()
    }

    // Normally, We will always try to send as long as the kernel send buf is not empty.
    // However, if the user calls close, we will wait LINGER time
    // and then cancel on-going or new-issued send requests.
    pub async fn try_clear_msg_queue_when_close(&self) {
        let inner = self.inner.lock().unwrap();
        debug_assert!(inner.is_shutdown());
        if inner.msg_queue.is_empty() {
            return;
        }

        // Wait for linger time to empty the kernel buffer or cancel subsequent requests.
        drop(inner);
        const DEFUALT_LINGER_TIME: usize = 10;
        let poller = Poller::new();
        let mask = Events::ERR | Events::OUT;
        self.common.pollee().connect_poller(mask, &poller);

        loop {
            let pending_request_exist = {
                let inner = self.inner.lock().unwrap();
                inner.io_handle.is_some()
            };

            if pending_request_exist {
                let mut timeout = Some(Duration::from_secs(DEFUALT_LINGER_TIME as u64));
                let ret = poller.wait_timeout(timeout.as_mut()).await;
                trace!("wait empty send buffer ret = {:?}", ret);
                if let Err(_) = ret {
                    // No complete request to wake. Just cancel the send requests.
                    let io_uring = self.common.io_uring();
                    let inner = self.inner.lock().unwrap();
                    if let Some(io_handle) = &inner.io_handle {
                        unsafe { io_uring.cancel(io_handle) };
                        // Loop again to wait the cancel request to complete
                        continue;
                    } else {
                        // No pending request, just break
                        break;
                    }
                }
            } else {
                // There is no pending requests
                break;
            }
        }
    }

    pub async fn sendmsg(
        self: &Arc<Self>,
        bufs: &[&[u8]],
        addr: &A,
        flags: SendFlags,
        control: Option<&[u8]>,
    ) -> Result<usize> {
        if !flags.is_empty()
            && flags.intersects(!(SendFlags::MSG_DONTWAIT | SendFlags::MSG_NOSIGNAL))
        {
            error!("Not supported flags: {:?}", flags);
            return_errno!(EINVAL, "not supported flags");
        }
        let mask = Events::OUT;
        // Initialize the poller only when needed
        let mut poller = None;
        let mut timeout = self.common.send_timeout();
        loop {
            // Attempt to write
            let res = self.try_sendmsg(bufs, addr, control);
            if !res.has_errno(EAGAIN) {
                return res;
            }

            // Still some buffer contents pending
            if self.common.nonblocking() || flags.contains(SendFlags::MSG_DONTWAIT) {
                return_errno!(EAGAIN, "try write again");
            }

            // Wait for interesting events by polling
            if poller.is_none() {
                let new_poller = Poller::new();
                self.common.pollee().connect_poller(mask, &new_poller);
                poller = Some(new_poller);
            }

            let events = self.common.pollee().poll(mask, None);
            if events.is_empty() {
                let ret = poller
                    .as_ref()
                    .unwrap()
                    .wait_timeout(timeout.as_mut())
                    .await;
                if let Err(e) = ret {
                    warn!("send wait errno = {:?}", e.errno());
                    match e.errno() {
                        ETIMEDOUT => {
                            return_errno!(EAGAIN, "timeout reached")
                        }
                        _ => {
                            return_errno!(e.errno(), "wait error")
                        }
                    }
                }
            }
        }
    }

    fn try_sendmsg(
        self: &Arc<Self>,
        bufs: &[&[u8]],
        addr: &A,
        control: Option<&[u8]>,
    ) -> Result<usize> {
        let mut inner = self.inner.lock().unwrap();
        if inner.is_shutdown() {
            return_errno!(EPIPE, "the write has been shutdown")
        }

        if let Some(errno) = inner.error {
            // Reset error
            inner.error = None;
            self.common.pollee().del_events(Events::ERR);
            return_errno!(errno, "write failed");
        }

        let buf_len: usize = bufs.iter().map(|buf| buf.len()).sum();
        let mut msg = DataMsg::new(buf_len);
        let total_copied = msg.copy_buf(bufs)?;
        msg.copy_control(control)?;

        let msghdr_ptr = new_send_req(&mut msg, addr);

        if !inner.msg_queue.push_msg(msg) {
            // Msg queue can not push this msg, mark the socket as non-writable
            self.common.pollee().del_events(Events::OUT);
            return_errno!(EAGAIN, "try write again");
        }

        // Since the send buffer is not empty, try to flush the buffer
        if inner.io_handle.is_none() {
            self.do_send(&mut inner, msghdr_ptr);
        }
        Ok(total_copied)
    }

    fn do_send(self: &Arc<Self>, inner: &mut MutexGuard<Inner>, msghdr_ptr: *const libc::msghdr) {
        debug_assert!(!inner.msg_queue.is_empty());
        debug_assert!(inner.io_handle.is_none());
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
            inner.msg_queue.pop_msg();
            sender.common.pollee().add_events(Events::OUT);
            if !inner.msg_queue.is_empty() {
                let msghdr_ptr = inner.msg_queue.first_msg_ptr();
                debug_assert!(msghdr_ptr.is_some());
                sender.do_send(&mut inner, msghdr_ptr.unwrap());
            } else if inner.is_shutdown == ShutdownStatus::PreShutdown {
                // The buffer is empty and the write side is shutdown by the user.
                // We can safely shutdown host file here.
                if A::domain() != Domain::Netlink {
                    let _ = sender.common.host_shutdown(Shutdown::Write);
                }
                inner.is_shutdown = ShutdownStatus::PostShutdown
            }
        };

        // Generate the async recv request
        let io_uring = self.common.io_uring();
        let host_fd = Fd(self.common.host_fd() as _);
        let handle = unsafe { io_uring.sendmsg(host_fd, msghdr_ptr, 0, complete_fn) };
        inner.io_handle.replace(handle);
    }
}

fn new_send_req<A: Addr>(dmsg: &mut DataMsg, addr: &A) -> *const libc::msghdr {
    let iovec = libc::iovec {
        iov_base: dmsg.send_buf.as_ptr() as _,
        iov_len: dmsg.send_buf.len(),
    };

    let (control, controllen) = match &dmsg.control {
        Some(control) => (control.as_mut_ptr() as *mut c_void, control.len()),
        None => (ptr::null_mut(), 0),
    };

    dmsg.req.iovec = iovec;

    dmsg.req.msg.msg_iov = &raw mut dmsg.req.iovec as _;
    dmsg.req.msg.msg_iovlen = 1;

    let (c_addr_storage, c_addr_len) = addr.to_c_storage();

    dmsg.req.addr = c_addr_storage;
    dmsg.req.msg.msg_name = &raw mut dmsg.req.addr as _;
    dmsg.req.msg.msg_namelen = c_addr_len as _;
    dmsg.req.msg.msg_control = control;
    dmsg.req.msg.msg_controllen = controllen;

    &mut dmsg.req.msg
}

pub struct Inner {
    io_handle: Option<IoHandle>,
    error: Option<Errno>,
    is_shutdown: ShutdownStatus,
    msg_queue: MsgQueue,
}

unsafe impl Send for Inner {}

impl Inner {
    pub fn new() -> Self {
        Self {
            io_handle: None,
            error: None,
            is_shutdown: ShutdownStatus::Running,
            msg_queue: MsgQueue::new(),
        }
    }

    /// Obtain udp sender shutdown state.
    #[inline(always)]
    pub fn is_shutdown(&self) -> bool {
        self.is_shutdown == ShutdownStatus::PreShutdown
            || self.is_shutdown == ShutdownStatus::PostShutdown
    }
}

#[repr(C)]
struct SendReq {
    msg: libc::msghdr,
    iovec: libc::iovec,
    addr: libc::sockaddr_storage,
}

unsafe impl MaybeUntrusted for SendReq {}

struct MsgQueue {
    queue: VecDeque<DataMsg>,
    curr_size: usize,
}

impl MsgQueue {
    #[inline(always)]
    fn new() -> Self {
        Self {
            queue: VecDeque::with_capacity(SENDMSG_QUEUE_LEN),
            curr_size: 0,
        }
    }

    #[inline(always)]
    fn size(&self) -> usize {
        self.curr_size
    }

    #[inline(always)]
    fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    // Push datagram msg, return true if succeed,
    // return false if buffer is full.
    #[inline(always)]
    fn push_msg(&mut self, msg: DataMsg) -> bool {
        let total_len = msg.len() + self.size();
        if total_len <= super::MAX_BUF_SIZE {
            self.curr_size = total_len;
            self.queue.push_back(msg);
            return true;
        }
        false
    }

    #[inline(always)]
    fn pop_msg(&mut self) {
        if let Some(msg) = self.queue.pop_front() {
            self.curr_size = self.size() - msg.len();
        }
    }

    #[inline(always)]
    fn first_msg_ptr(&self) -> Option<*const libc::msghdr> {
        self.queue
            .front()
            .map(|data_msg| &data_msg.req.msg as *const libc::msghdr)
    }
}

// Datagram msg contents in untrusted region
struct DataMsg {
    req: UntrustedBox<SendReq>,
    send_buf: UntrustedBox<[u8]>,
    control: Option<UntrustedBox<[u8]>>,
}

impl DataMsg {
    #[inline(always)]
    fn new(buf_len: usize) -> Self {
        Self {
            req: UntrustedBox::<SendReq>::new_uninit(),
            send_buf: UntrustedBox::new_uninit_slice(buf_len),
            control: None,
        }
    }

    #[inline(always)]
    fn copy_buf(&mut self, bufs: &[&[u8]]) -> Result<usize> {
        let total_len = self.send_buf.len();
        if total_len > super::MAX_BUF_SIZE {
            return_errno!(EMSGSIZE, "the message is too large")
        }
        // Copy data from the bufs to the send buffer
        let mut total_copied = 0;
        for buf in bufs {
            self.send_buf[total_copied..(total_copied + buf.len())].copy_from_slice(buf);
            total_copied += buf.len();
        }
        Ok(total_copied)
    }

    #[inline(always)]
    fn copy_control(&mut self, control: Option<&[u8]>) -> Result<usize> {
        if let Some(msg_control) = control {
            let send_controllen = msg_control.len();
            if send_controllen > super::OPTMEM_MAX {
                return_errno!(EINVAL, "invalid msg control length");
            }
            let mut send_control_buf = UntrustedBox::new_uninit_slice(send_controllen);
            send_control_buf.copy_from_slice(&msg_control[..send_controllen]);

            self.control = Some(send_control_buf);
            return Ok(send_controllen);
        };
        Ok(0)
    }

    #[inline(always)]
    fn len(&self) -> usize {
        self.send_buf.len()
    }
}

#[derive(Debug, PartialEq)]
enum ShutdownStatus {
    Running,      // not shutdown
    PreShutdown,  // start the shutdown process, set by calling shutdown syscall
    PostShutdown, // shutdown process is done, set when the buffer is empty
}
