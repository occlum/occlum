use std::mem::MaybeUninit;
use std::ptr::{self};

use io_uring_callback::{Fd, IoHandle};
use sgx_untrusted_alloc::{MaybeUntrusted, UntrustedBox};

use super::ConnectedStream;
use crate::prelude::*;
use crate::runtime::Runtime;
use crate::util::UntrustedCircularBuf;

impl<A: Addr + 'static, R: Runtime> ConnectedStream<A, R> {
    pub async fn recvmsg(
        self: &Arc<Self>,
        bufs: &mut [&mut [u8]],
        flags: RecvFlags,
    ) -> Result<usize> {
        let total_len: usize = bufs.iter().map(|buf| buf.len()).sum();
        if total_len == 0 {
            return Ok(0);
        }

        let mut total_received = 0;
        let mut iov_buffer_index = 0;
        let mut iov_buffer_offset = 0;

        let mask = Events::IN;
        // Initialize the poller only when needed
        let mut poller = None;
        let mut timeout = self.common.recv_timeout();
        loop {
            // Attempt to read
            let res = self.try_recvmsg(bufs, flags, iov_buffer_index, iov_buffer_offset);

            match res {
                Ok((received_size, index, offset)) => {
                    total_received += received_size;

                    if !flags.contains(RecvFlags::MSG_WAITALL) || total_received == total_len {
                        return Ok(total_received);
                    } else {
                        // save the index and offset for the next round
                        iov_buffer_index = index;
                        iov_buffer_offset = offset;
                    }
                }
                Err(e) => {
                    if e.errno() != EAGAIN {
                        return Err(e);
                    }
                }
            };

            if self.common.nonblocking() || flags.contains(RecvFlags::MSG_DONTWAIT) {
                return_errno!(EAGAIN, "no data are present to be received");
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
                    warn!("recv wait errno = {:?}", e.errno());
                    // For recv with MSG_WAITALL, return total received bytes if timeout or interrupt
                    if flags.contains(RecvFlags::MSG_WAITALL) && total_received > 0 {
                        return Ok(total_received);
                    }
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

    fn try_recvmsg(
        self: &Arc<Self>,
        bufs: &mut [&mut [u8]],
        flags: RecvFlags,
        iov_buffer_index: usize,
        iov_buffer_offset: usize,
    ) -> Result<(usize, usize, usize)> {
        let mut inner = self.receiver.inner.lock().unwrap();

        if !flags.is_empty()
            && flags.intersects(!(RecvFlags::MSG_DONTWAIT | RecvFlags::MSG_WAITALL))
        {
            warn!("Unsupported flags: {:?}", flags);
            return_errno!(EINVAL, "flags not supported");
        }

        let res = {
            let mut total_consumed = 0;
            let mut iov_buffer_index = iov_buffer_index;
            let mut iov_buffer_offset = iov_buffer_offset;

            // save the received data from bufs[iov_buffer_index][iov_buffer_offset..]
            for (_, buf) in bufs.iter_mut().skip(iov_buffer_index).enumerate() {
                let this_consumed = inner.recv_buf.consume(&mut buf[iov_buffer_offset..]);
                if this_consumed == 0 {
                    break;
                }
                total_consumed += this_consumed;

                // if the buffer is not full, then the try_recvmsg will be used again
                // next time, the data will be stored from the offset
                if this_consumed < buf[iov_buffer_offset..].len() {
                    iov_buffer_offset += this_consumed;
                    break;
                } else {
                    iov_buffer_index += 1;
                    iov_buffer_offset = 0;
                }
            }
            (total_consumed, iov_buffer_index, iov_buffer_offset)
        };

        if inner.end_of_file {
            return Ok(res);
        }

        if inner.recv_buf.is_empty() {
            // Mark the socket as non-readable
            self.common.pollee().del_events(Events::IN);
        }

        if res.0 > 0 {
            self.do_recv(&mut inner);
            return Ok(res);
        }

        // Only when there are no data available in the recv buffer, shall we check
        // the following error conditions.
        //
        // Case 1: If the read side of the connection has been shutdown...
        if inner.is_shutdown {
            return_errno!(EPIPE, "read side is shutdown");
        }
        // Case 2: If the connenction has been broken...
        if let Some(errno) = inner.fatal {
            // Reset error
            inner.fatal = None;
            self.common.pollee().del_events(Events::ERR);
            return_errno!(errno, "read failed");
        }

        self.do_recv(&mut inner);
        return_errno!(EAGAIN, "try read again");
    }

    fn do_recv(self: &Arc<Self>, inner: &mut MutexGuard<Inner>) {
        if inner.recv_buf.is_full()
            || inner.is_shutdown
            || inner.io_handle.is_some()
            || inner.end_of_file
            || self.common.is_closed()
        {
            // Delete ERR events from sender. If io_handle is some, the recv request must be
            // pending and the events can't be for the reciever. Just delete this event.
            // This can happen when send request is timeout and canceled.
            let events = self.common.pollee().poll(Events::IN, None);
            if events.contains(Events::ERR) && inner.io_handle.is_some() {
                self.common.pollee().del_events(Events::ERR);
            }
            return;
        }

        // Init the callback invoked upon the completion of the async recv
        let stream = self.clone();
        let complete_fn = move |retval: i32| {
            let mut inner = stream.receiver.inner.lock().unwrap();
            trace!("recv request complete with retval: {}", retval);

            // Release the handle to the async recv
            inner.io_handle.take();

            // Handle error
            if retval < 0 {
                // TODO: guard against Iago attack through errno
                // We should return here, The error may be due to network reasons
                // or because the request was cancelled. We don't want to start a
                // new request after cancelled a request.
                let errno = Errno::from(-retval as u32);
                inner.fatal = Some(errno);
                stream.common.pollee().add_events(Events::ERR);
                return;
            }
            // Handle end of file
            else if retval == 0 {
                inner.end_of_file = true;
                stream.common.pollee().add_events(Events::IN);
                return;
            }

            // Handle the normal case of a successful read
            let nbytes = retval as usize;
            inner.recv_buf.produce_without_copy(nbytes);

            // Now that we have produced non-zero bytes, the buf must become
            // ready to read.
            stream.common.pollee().add_events(Events::IN);

            stream.do_recv(&mut inner);
        };

        // Generate the async recv request
        let msghdr_ptr = inner.new_recv_req();

        // Submit the async recv to io_uring
        let io_uring = self.common.io_uring();
        let host_fd = Fd(self.common.host_fd() as _);
        let handle = unsafe { io_uring.recvmsg(host_fd, msghdr_ptr, 0, complete_fn) };
        inner.io_handle.replace(handle);
    }

    pub(super) fn initiate_async_recv(self: &Arc<Self>) {
        // trace!("initiate async recv");
        let mut inner = self.receiver.inner.lock().unwrap();
        self.do_recv(&mut inner);
    }

    pub async fn cancel_recv_requests(&self) {
        {
            let inner = self.receiver.inner.lock().unwrap();
            if let Some(io_handle) = &inner.io_handle {
                let io_uring = self.common.io_uring();
                unsafe { io_uring.cancel(io_handle) };
            } else {
                return;
            }
        }

        // wait for the cancel to complete
        let poller = Poller::new();
        let mask = Events::ERR | Events::IN;
        self.common.pollee().connect_poller(mask, &poller);

        loop {
            let pending_request_exist = {
                let inner = self.receiver.inner.lock().unwrap();
                inner.io_handle.is_some()
            };

            if pending_request_exist {
                let mut timeout = Some(Duration::from_secs(10));
                let ret = poller.wait_timeout(timeout.as_mut()).await;
                if let Err(e) = ret {
                    warn!("wait cancel recv request error = {:?}", e.errno());
                    continue;
                }
            } else {
                break;
            }
        }
    }

    pub fn bytes_to_consume(self: &Arc<Self>) -> usize {
        let inner = self.receiver.inner.lock().unwrap();
        inner.recv_buf.consumable()
    }
}

pub struct Receiver {
    inner: Mutex<Inner>,
}

impl Receiver {
    pub fn new() -> Self {
        let inner = Mutex::new(Inner::new());
        Self { inner }
    }

    pub fn shutdown(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.is_shutdown = true;
    }
}

impl std::fmt::Debug for Receiver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Receiver")
            .field("inner", &self.inner.lock().unwrap())
            .finish()
    }
}

struct Inner {
    recv_buf: UntrustedCircularBuf,
    recv_req: UntrustedBox<RecvReq>,
    io_handle: Option<IoHandle>,
    is_shutdown: bool,
    end_of_file: bool,
    fatal: Option<Errno>,
}

// Safety. `RecvReq` does not implement `Send`. But since all pointers in `RecvReq`
// refer to `recv_buf`, we can be sure that it is ok for `RecvReq` to move between
// threads. All other fields in `RecvReq` implement `Send` as well. So the entirety
// of `Inner` is `Send`-safe.
unsafe impl Send for Inner {}

impl Inner {
    pub fn new() -> Self {
        Self {
            recv_buf: UntrustedCircularBuf::with_capacity(super::RECV_BUF_SIZE),
            recv_req: UntrustedBox::new_uninit(),
            io_handle: None,
            is_shutdown: false,
            end_of_file: false,
            fatal: None,
        }
    }

    /// Constructs a new recv request according to the receiver's internal state.
    ///
    /// The new `RecvReq` will be put into `self.recv_req`, which is a location that is
    /// accessible by io_uring. A pointer to the C version of the resulting `RecvReq`,
    /// which is `libc::msghdr`, will be returned.
    ///
    /// The buffer used in the new `RecvReq` is part of `self.recv_buf`.
    pub fn new_recv_req(&mut self) -> *mut libc::msghdr {
        let (iovecs, iovecs_len) = self.gen_iovecs_from_recv_buf();

        let msghdr_ptr: *mut libc::msghdr = &mut self.recv_req.msg;
        let iovecs_ptr: *mut libc::iovec = &mut self.recv_req.iovecs as *mut _ as _;

        let msg = super::new_msghdr(iovecs_ptr, iovecs_len);

        self.recv_req.msg = msg;
        self.recv_req.iovecs = iovecs;

        msghdr_ptr
    }

    fn gen_iovecs_from_recv_buf(&mut self) -> ([libc::iovec; 2], usize) {
        let mut iovecs_len = 0;
        let mut iovecs = unsafe { MaybeUninit::<[libc::iovec; 2]>::uninit().assume_init() };
        self.recv_buf.with_producer_view(|part0, part1| {
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

            // Only access the producer's buffer; zero bytes produced for now.
            0
        });
        debug_assert!(iovecs_len > 0);
        (iovecs, iovecs_len)
    }
}

impl std::fmt::Debug for Inner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Inner")
            .field("recv_buf", &self.recv_buf)
            .field("io_handle", &self.io_handle)
            .field("is_shutdown", &self.is_shutdown)
            .field("end_of_file", &self.end_of_file)
            .field("fatal", &self.fatal)
            .finish()
    }
}

#[repr(C)]
struct RecvReq {
    msg: libc::msghdr,
    iovecs: [libc::iovec; 2],
}

// Safety. RecvReq is a C-style struct.
unsafe impl MaybeUntrusted for RecvReq {}

// Acquired by `IoUringCell<T: Copy>`.
impl Copy for RecvReq {}

impl Clone for RecvReq {
    fn clone(&self) -> Self {
        *self
    }
}
