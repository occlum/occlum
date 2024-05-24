use core::hint;
use core::sync::atomic::AtomicBool;
use core::time::Duration;
use std::mem::MaybeUninit;
use std::ptr::{self};

use atomic::Ordering;
use io_uring_callback::{Fd, IoHandle};
use log::error;
use sgx_untrusted_alloc::{MaybeUntrusted, UntrustedBox};

use super::ConnectedStream;
use crate::net::socket::uring::runtime::Runtime;
use crate::net::socket::uring::stream::SEND_BUF_SIZE;
use crate::prelude::*;
use crate::untrusted::UntrustedCircularBuf;

use crate::util::sync::{Mutex, MutexGuard};

use crate::events::Poller;
use crate::fs::IoEvents as Events;

impl<A: Addr + 'static, R: Runtime> ConnectedStream<A, R> {
    // We make sure the all the buffer contents are buffered in kernel and then return.
    pub fn sendmsg(self: &Arc<Self>, bufs: &[&[u8]], flags: SendFlags) -> Result<usize> {
        let total_len: usize = bufs.iter().map(|buf| buf.len()).sum();
        if total_len == 0 {
            return Ok(0);
        }

        let mut send_len = 0;
        // variables to track the position of async sendmsg.
        let mut iov_buf_id = 0; // user buffer id tracker
        let mut iov_buf_index = 0; // user buffer index tracker

        let mask = Events::OUT;
        // Initialize the poller only when needed
        let mut poller = None;
        let mut timeout = self.common.send_timeout();
        loop {
            // Attempt to write
            let res = self.try_sendmsg(bufs, flags, &mut iov_buf_id, &mut iov_buf_index);
            if let Ok(len) = res {
                send_len += len;
                // Sent all or sent partial but it is nonblocking, return bytes sent
                if send_len == total_len
                    || self.common.nonblocking()
                    || flags.contains(SendFlags::MSG_DONTWAIT)
                {
                    return Ok(send_len);
                }
            } else if !res.has_errno(EAGAIN) {
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
                let ret = poller.as_ref().unwrap().wait_timeout(timeout.as_mut());
                if let Err(e) = ret {
                    warn!("send wait errno = {:?}", e.errno());
                    match e.errno() {
                        ETIMEDOUT => {
                            // Just cancel send requests if timeout
                            self.cancel_send_requests();
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
        flags: SendFlags,
        iov_buf_id: &mut usize,
        iov_buf_index: &mut usize,
    ) -> Result<usize> {
        let mut inner = self.sender.inner.lock();

        if !flags.is_empty()
            && flags.intersects(
                !(SendFlags::MSG_DONTWAIT | SendFlags::MSG_NOSIGNAL | SendFlags::MSG_MORE),
            )
        {
            error!("Not supported flags: {:?}", flags);
            return_errno!(EINVAL, "not supported flags");
        }

        // Check for error condition before write.
        //
        // Case 1. If the write side of the connection has been shutdown...
        if inner.is_shutdown() {
            return_errno!(EPIPE, "write side is shutdown");
        }
        // Case 2. If the connenction has been broken...
        if let Some(errno) = inner.fatal {
            // Reset error
            inner.fatal = None;
            self.common.pollee().del_events(Events::ERR);
            return_errno!(errno, "write failed");
        }

        // Copy data from the bufs to the send buffer
        // If the send buffer is full, update the user buffer tracker, return error to wait for events
        // And once there is free space, continue from the user buffer tracker
        let nbytes = {
            let mut total_produced = 0;
            let last_time_buf_id = iov_buf_id.clone();
            let mut last_time_buf_idx = iov_buf_index.clone();
            for (_i, buf) in bufs.iter().skip(last_time_buf_id).enumerate() {
                let i = _i + last_time_buf_id; // After skipping ,the index still starts from 0
                let this_produced = inner.send_buf.produce(&buf[last_time_buf_idx..]);
                total_produced += this_produced;
                if this_produced < buf[last_time_buf_idx..].len() {
                    // Send buffer is full.
                    *iov_buf_id = i;
                    *iov_buf_index = last_time_buf_idx + this_produced;
                    break;
                } else {
                    // For next buffer, start from the front
                    last_time_buf_idx = 0;
                }
            }
            total_produced
        };

        if inner.send_buf.is_full() {
            // Mark the socket as non-writable
            self.common.pollee().del_events(Events::OUT);
        }

        // Since the send buffer is not empty, we can try to flush the buffer
        if inner.io_handle.is_none() {
            self.do_send(&mut inner);
        }

        if nbytes > 0 {
            Ok(nbytes)
        } else {
            return_errno!(EAGAIN, "try write again");
        }
    }

    fn do_send(self: &Arc<Self>, inner: &mut MutexGuard<Inner>) {
        // This function can also be called even if the socket is set to shutdown by shutdown syscall. This is due to the
        // async behaviour that the kernel may return to user before actually issuing the request. We should
        // keep sending the request as long as the send buffer is not empty even if the socket is shutdown.
        debug_assert!(inner.is_shutdown != ShutdownStatus::PostShutdown);
        debug_assert!(!inner.send_buf.is_empty());
        debug_assert!(inner.io_handle.is_none());

        // Init the callback invoked upon the completion of the async send
        let stream = self.clone();
        let complete_fn = move |retval: i32| {
            let mut inner = stream.sender.inner.lock();

            trace!("send request complete with retval: {}", retval);
            // Release the handle to the async send
            inner.io_handle.take();

            // Handle error
            if retval < 0 {
                // TODO: guard against Iago attack through errno
                // TODO: should we ignore EINTR and try again?
                let errno = Errno::from(-retval as u32);

                inner.fatal = Some(errno);
                stream.common.set_errno(errno);

                let events = if errno == ENOTCONN || errno == ECONNRESET || errno == ECONNREFUSED {
                    Events::HUP | Events::OUT | Events::ERR
                } else {
                    Events::ERR
                };

                stream.common.pollee().add_events(events);
                return;
            }
            assert!(retval != 0);

            // Handle the normal case of a successful write
            let nbytes = retval as usize;
            inner.send_buf.consume_without_copy(nbytes);

            // Now that we have consume non-zero bytes, the buf must become
            // ready to write.
            stream.common.pollee().add_events(Events::OUT);

            // Attempt to send again if there are available data in the buf.
            if !inner.send_buf.is_empty() {
                stream.do_send(&mut inner);
            } else if inner.is_shutdown == ShutdownStatus::PreShutdown {
                // The buffer is empty and the write side is shutdown by the user. We can safely shutdown host file here.
                let _ = stream.common.host_shutdown(Shutdown::Write);
                inner.is_shutdown = ShutdownStatus::PostShutdown
            } else if stream.sender.need_update() {
                // send_buf is empty. We can try to update the send_buf
                stream.sender.set_need_update(false);
                inner.update_buf_size(SEND_BUF_SIZE.load(Ordering::Relaxed));
            }
        };

        // Generate the async send request
        let msghdr_ptr = inner.new_send_req();

        trace!("send submit request");
        // Submit the async send to io_uring
        let io_uring = self.common.io_uring();
        let host_fd = Fd(self.common.host_fd() as _);
        let handle = unsafe { io_uring.sendmsg(host_fd, msghdr_ptr, 0, complete_fn) };
        inner.io_handle.replace(handle);
    }

    pub fn cancel_send_requests(&self) {
        let io_uring = self.common.io_uring();
        let inner = self.sender.inner.lock();
        if let Some(io_handle) = &inner.io_handle {
            unsafe { io_uring.cancel(io_handle) };
        }
    }

    // This function will try to update the kernel buf size.
    // If the kernel buf is currently empty, the size will be updated immediately.
    // If the kernel buf is not empty, update the flag in Sender and update the kernel buf after send.
    pub fn try_update_send_buf_size(&self, buf_size: usize) {
        let pre_buf_size = SEND_BUF_SIZE.swap(buf_size, Ordering::Relaxed);
        if pre_buf_size == buf_size {
            return;
        }

        // Try to acquire the lock. If success, try directly update here.
        // If failure, don't wait because there is pending send request.
        if let Some(mut inner) = self.sender.inner.try_lock() {
            if inner.send_buf.is_empty() && inner.io_handle.is_none() {
                inner.update_buf_size(buf_size);
                return;
            }
        }

        // Can't easily aquire lock or the sendbuf is not empty. Update the flag only
        self.sender.set_need_update(true);
    }

    // Normally, We will always try to send as long as the kernel send buf is not empty. However, if the user calls close, we will wait LINGER time
    // and then cancel on-going or new-issued send requests.
    pub fn try_empty_send_buf_when_close(&self) {
        // let inner = self.sender.inner.lock().unwrap();
        let inner = self.sender.inner.lock();
        debug_assert!(inner.is_shutdown());
        if inner.send_buf.is_empty() {
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
                // let inner = self.sender.inner.lock().unwrap();
                let inner = self.sender.inner.lock();
                inner.io_handle.is_some()
            };

            if pending_request_exist {
                let mut timeout = Some(Duration::from_secs(DEFUALT_LINGER_TIME as u64));
                let ret = poller.wait_timeout(timeout.as_mut());
                trace!("wait empty send buffer ret = {:?}", ret);
                if let Err(_) = ret {
                    // No complete request to wake. Just cancel the send requests.
                    let io_uring = self.common.io_uring();
                    let inner = self.sender.inner.lock();
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
}

pub struct Sender {
    inner: Mutex<Inner>,
    need_update: AtomicBool,
}

impl Sender {
    pub fn new() -> Self {
        let inner = Mutex::new(Inner::new());
        let need_update = AtomicBool::new(false);
        Self { inner, need_update }
    }

    pub fn shutdown(&self) {
        let mut inner = self.inner.lock();
        inner.is_shutdown = ShutdownStatus::PreShutdown;
    }

    pub fn is_empty(&self) -> bool {
        let inner = self.inner.lock();
        inner.send_buf.is_empty()
    }

    pub fn set_need_update(&self, need_update: bool) {
        self.need_update.store(need_update, Ordering::Relaxed)
    }

    pub fn need_update(&self) -> bool {
        self.need_update.load(Ordering::Relaxed)
    }
}

impl std::fmt::Debug for Sender {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sender")
            .field("inner", &self.inner.lock())
            .finish()
    }
}

struct Inner {
    send_buf: UntrustedCircularBuf,
    send_req: UntrustedBox<SendReq>,
    io_handle: Option<IoHandle>,
    is_shutdown: ShutdownStatus,
    fatal: Option<Errno>,
}

// Safety. `SendReq` does not implement `Send`. But since all pointers in `SengReq`
// refer to `send_buf`, we can be sure that it is ok for `SendReq` to move between
// threads. All other fields in `SendReq` implement `Send` as well. So the entirety
// of `Inner` is `Send`-safe.
unsafe impl Send for Inner {}

impl Inner {
    pub fn new() -> Self {
        Self {
            send_buf: UntrustedCircularBuf::with_capacity(SEND_BUF_SIZE.load(Ordering::Relaxed)),
            send_req: UntrustedBox::new_uninit(),
            io_handle: None,
            is_shutdown: ShutdownStatus::Running,
            fatal: None,
        }
    }

    fn update_buf_size(&mut self, buf_size: usize) {
        debug_assert!(self.send_buf.is_empty() && self.io_handle.is_none());
        let new_send_buf = UntrustedCircularBuf::with_capacity(buf_size);
        self.send_buf = new_send_buf;
    }

    pub fn is_shutdown(&self) -> bool {
        self.is_shutdown == ShutdownStatus::PreShutdown
            || self.is_shutdown == ShutdownStatus::PostShutdown
    }

    /// Constructs a new send request according to the sender's internal state.
    ///
    /// The new `SendReq` will be put into `self.send_req`, which is a location that is
    /// accessible by io_uring. A pointer to the C version of the resulting `SendReq`,
    /// which is `libc::msghdr`, will be returned.
    ///
    /// The buffer used in the new `SendReq` is part of `self.send_buf`.
    pub fn new_send_req(&mut self) -> *mut libc::msghdr {
        let (iovecs, iovecs_len) = self.gen_iovecs_from_send_buf();

        let msghdr_ptr: *mut libc::msghdr = &mut self.send_req.msg;
        let iovecs_ptr: *mut libc::iovec = &mut self.send_req.iovecs as *mut _ as _;

        let msg = super::new_msghdr(iovecs_ptr, iovecs_len);

        self.send_req.msg = msg;
        self.send_req.iovecs = iovecs;

        msghdr_ptr
    }

    fn gen_iovecs_from_send_buf(&mut self) -> ([libc::iovec; 2], usize) {
        let mut iovecs_len = 0;
        let mut iovecs = unsafe { MaybeUninit::<[libc::iovec; 2]>::uninit().assume_init() };
        self.send_buf.with_consumer_view(|part0, part1| {
            debug_assert!(part0.len() > 0);

            iovecs[0] = libc::iovec {
                iov_base: part0.as_ptr() as _,
                iov_len: part0.len() as _,
            };

            iovecs[1] = if part1.len() > 0 {
                iovecs_len = 2;
                libc::iovec {
                    iov_base: part1.as_ptr() as _,
                    iov_len: part1.len() as _,
                }
            } else {
                iovecs_len = 1;
                libc::iovec {
                    iov_base: ptr::null_mut(),
                    iov_len: 0,
                }
            };

            // Only access the consumer's buffer; zero bytes consumed for now.
            0
        });
        debug_assert!(iovecs_len > 0);
        (iovecs, iovecs_len)
    }
}

impl std::fmt::Debug for Inner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Inner")
            .field("send_buf", &self.send_buf)
            .field("io_handle", &self.io_handle)
            .field("is_shutdown", &self.is_shutdown)
            .field("fatal", &self.fatal)
            .finish()
    }
}

#[repr(C)]
struct SendReq {
    msg: libc::msghdr,
    iovecs: [libc::iovec; 2],
}

// Safety. SendReq is a C-style struct.
unsafe impl MaybeUntrusted for SendReq {}

// Acquired by `IoUringCell<T: Copy>`.
impl Copy for SendReq {}

impl Clone for SendReq {
    fn clone(&self) -> Self {
        *self
    }
}

#[derive(Debug, PartialEq)]
enum ShutdownStatus {
    Running,      // not shutdown
    PreShutdown,  // start the shutdown process, set by calling shutdown syscall
    PostShutdown, // shutdown process is done, set when the buffer is empty
}
