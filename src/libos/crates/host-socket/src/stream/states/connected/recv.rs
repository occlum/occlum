use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::ptr::{self};

use async_io::poll::{Events, Poller};
use async_io::socket::Addr;
use io_uring_callback::{Fd, IoHandle, IoUringArray, IoUringCell};
use memoffset::offset_of;

use super::ConnectedStream;
use crate::prelude::*;
use crate::runtime::Runtime;
use crate::util::CircularBuf;

impl<A: Addr + 'static, R: Runtime> ConnectedStream<A, R> {
    pub async fn read(self: &Arc<Self>, buf: &mut [u8]) -> Result<usize> {
        if buf.len() == 0 {
            return Ok(0);
        }

        // Initialize the poller only when needed
        let mut poller = None;
        loop {
            // Attempt to reade
            let res = self.try_read(buf);
            if !res.has_errno(EAGAIN) {
                return res;
            }

            // Wait for interesting events by polling
            if poller.is_none() {
                poller = Some(Poller::new());
            }
            let mask = Events::IN;
            let events = self.common.pollee().poll_by(mask, poller.as_mut());
            if events.is_empty() {
                poller.as_ref().unwrap().wait().await;
            }
        }
    }

    fn try_read(self: &Arc<Self>, buf: &mut [u8]) -> Result<usize> {
        let mut inner = self.receiver.inner.lock().unwrap();

        let nbytes = inner.recv_buf.consume(buf);

        if inner.end_of_file {
            return Ok(nbytes);
        }

        if inner.recv_buf.is_empty() {
            // Mark the socket as non-readable
            self.common.pollee().del_events(Events::IN);
        }

        if nbytes > 0 {
            self.do_recv(&mut inner);
            return Ok(nbytes);
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
        {
            return;
        }

        // Init the callback invoked upon the completion of the async recv
        let stream = self.clone();
        let complete_fn = move |retval: i32| {
            let mut inner = stream.receiver.inner.lock().unwrap();

            // Release the handle to the async recv
            inner.io_handle.take();

            // Handle error
            if retval < 0 {
                // TODO: guard against Iago attack through errno
                // TODO: should we ignore EINTR and try again?
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
        // TODO: update pollee?
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
    recv_buf: CircularBuf<IoUringArray<u8>>,
    recv_req: IoUringCell<RecvReq>,
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
            recv_buf: CircularBuf::new(IoUringArray::with_capacity(super::RECV_BUF_SIZE)),
            recv_req: IoUringCell::new(unsafe { MaybeUninit::<RecvReq>::uninit().assume_init() }),
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

        let (msghdr_ptr, iovecs_ptr) = {
            let recv_req_ptr = self.recv_req.as_ptr() as *mut u8;
            let msghdr_ptr = unsafe { recv_req_ptr.add(offset_of!(RecvReq, msg)) };
            let iovecs_ptr = unsafe { recv_req_ptr.add(offset_of!(RecvReq, iovecs)) };
            (
                msghdr_ptr as *mut libc::msghdr,
                iovecs_ptr as *mut libc::iovec,
            )
        };

        let msg = libc::msghdr {
            msg_name: ptr::null_mut() as _,
            msg_namelen: 0,
            msg_iov: iovecs_ptr,
            msg_iovlen: iovecs_len,
            msg_control: ptr::null_mut() as _,
            msg_controllen: 0,
            msg_flags: 0,
        };

        let new_recv_req = RecvReq { msg, iovecs };
        self.recv_req.set(new_recv_req);

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

// Acquired by `IoUringCell<T: Copy>`.
impl Copy for RecvReq {}

impl Clone for RecvReq {
    fn clone(&self) -> Self {
        *self
    }
}
