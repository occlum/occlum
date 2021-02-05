use std::mem::ManuallyDrop;
#[cfg(feature = "sgx")]
use std::prelude::v1::*;
use std::ptr::{self, NonNull};
#[cfg(not(feature = "sgx"))]
use std::sync::{Arc, Mutex, MutexGuard};
#[cfg(feature = "sgx")]
use std::sync::{Arc, SgxMutex as Mutex, SgxMutexGuard as MutexGuard};

use io_uring_callback::{Fd, Handle};
#[cfg(feature = "sgx")]
use sgx_untrusted_alloc::UntrustedAllocator;

use crate::io::{Common, IoUringProvider};
use crate::poll::{Events, Poller};
use crate::util::CircularBuf;

pub struct Receiver<P: IoUringProvider> {
    common: Arc<Common<P>>,
    inner: Mutex<Inner>,
}

struct Inner {
    buf: ManuallyDrop<CircularBuf>,
    #[cfg(not(feature = "sgx"))]
    buf_alloc: ManuallyDrop<Vec<u8>>,
    #[cfg(feature = "sgx")]
    buf_alloc: ManuallyDrop<UntrustedAllocator>,
    pending_io: Option<Handle>,
    end_of_file: bool,
    msg_param: ManuallyDrop<*mut MsgParam>,
    #[cfg(feature = "sgx")]
    msg_param_alloc: ManuallyDrop<UntrustedAllocator>,
}

unsafe impl Send for Inner {}

// Contains msghdr and iovec. Not support msg_name and and msg_control.
pub(crate) struct MsgParam {
    pub msg: libc::msghdr,
    pub iovecs: [libc::iovec; 2],
}

impl<P: IoUringProvider> Receiver<P> {
    /// Construct the receiver of a socket.
    pub(crate) fn new(common: Arc<Common<P>>, buf_size: usize) -> Arc<Self> {
        let new_self = {
            let inner = Mutex::new(Inner::new(buf_size));
            Arc::new(Self { common, inner })
        };

        {
            let mut inner = new_self.inner.lock().unwrap();
            new_self.fill_buf(&mut inner);
        }

        new_self
    }

    pub async fn read(self: &Arc<Self>, buf: &mut [u8]) -> i32 {
        // Initialize the poller only when needed
        let mut poller = None;
        loop {
            // Attempt to read
            let ret = self.try_read(buf);
            if ret != -libc::EAGAIN {
                return ret;
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

    fn try_read(self: &Arc<Self>, buf: &mut [u8]) -> i32 {
        let mut inner = self.inner.lock().unwrap();

        if buf.len() == 0 {
            return 0;
        }

        let nbytes = inner.buf.consume(buf);

        if inner.buf.is_empty() {
            // Mark the socket as non-readable
            self.common.pollee().remove(Events::IN);
        }

        if inner.end_of_file {
            return nbytes as i32;
        }

        if nbytes == 0 {
            if let Some(error) = self.common.error() {
                return error;
            }
            return -libc::EAGAIN;
        }

        if inner.pending_io.is_none() {
            self.fill_buf(&mut inner);
        }
        nbytes as i32
    }

    fn fill_buf(self: &Arc<Self>, inner: &mut MutexGuard<Inner>) {
        debug_assert!(!inner.buf.is_full());
        debug_assert!(!inner.end_of_file);
        debug_assert!(inner.pending_io.is_none());

        // Init the callback invoked upon the completion of the async fill
        let receiver = self.clone();
        let complete_fn = move |retval: i32| {
            let mut inner = receiver.inner.lock().unwrap();

            // Release the handle to the async fill
            inner.pending_io.take();

            // Handle the two special cases of error and "end-of-file"
            if retval < 0 {
                receiver.common.set_error(retval);
                receiver.common.pollee().add(Events::ERR);
                return;
            } else if retval == 0 {
                inner.end_of_file = true;
                receiver.common.pollee().add(Events::IN);
                return;
            }

            // Handle the normal case of a successful read
            let nbytes = retval as usize;
            inner.buf.produce_without_copy(nbytes);

            // Attempt to fill again if there are free space in the buf.
            if !inner.buf.is_full() {
                receiver.fill_buf(&mut inner);
            }

            // Now that we have produced non-zero bytes, the buf must become
            // ready to read.
            receiver.common.pollee().add(Events::IN);
        };

        // Construct the iovec for the async fill
        let mut iovec_len = 1;
        let msg_param_ptr = *inner.msg_param;
        let mut msg_ptr = unsafe { &mut (*msg_param_ptr).msg as *mut libc::msghdr };
        unsafe {
            inner.buf.with_producer_view(|part0, part1| {
                debug_assert!(part0.len() > 0);
                (*msg_param_ptr).iovecs[0] = libc::iovec {
                    iov_base: part0.as_ptr() as _,
                    iov_len: part0.len() as _,
                };

                if part1.len() > 0 {
                    (*msg_param_ptr).iovecs[1] = libc::iovec {
                        iov_base: part1.as_ptr() as _,
                        iov_len: part1.len() as _,
                    };
                    iovec_len += 1;
                }

                // Only access the producer's buffer; zero bytes produced for now.
                0
            });

            (*msg_ptr).msg_name = ptr::null_mut() as _;
            (*msg_ptr).msg_namelen = 0;
            (*msg_ptr).msg_iov = &mut (*msg_param_ptr).iovecs as *mut [libc::iovec; 2] as *mut _;
            (*msg_ptr).msg_iovlen = iovec_len;
            (*msg_ptr).msg_control = ptr::null_mut() as _;
            (*msg_ptr).msg_controllen = 0;
            (*msg_ptr).msg_flags = 0;
        }

        // Submit the async flush to io_uring
        let io_uring = &self.common.io_uring();
        let handle = unsafe { io_uring.recvmsg(Fd(self.common.fd()), msg_ptr, 0, complete_fn) };
        inner.pending_io.replace(handle);
    }

    /// Shutdown the receiver.
    ///
    /// After shutdowning, the receiver will no longer be able to receive more data, except
    /// those that are already received in our or OS's receive buffer.
    pub fn shutdown(&self) {
        let inner = self.inner.lock().unwrap();
        unsafe {
            #[cfg(not(feature = "sgx"))]
            libc::shutdown(self.common.fd(), libc::SHUT_RD);
            #[cfg(feature = "sgx")]
            libc::ocall::shutdown(self.common.fd(), libc::SHUT_RD);
        }
    }
}

impl Inner {
    pub fn new(buf_size: usize) -> Self {
        #[cfg(not(feature = "sgx"))]
        let mut buf_alloc = Vec::<u8>::with_capacity(buf_size);
        #[cfg(feature = "sgx")]
        let buf_alloc = UntrustedAllocator::new(buf_size, 1).unwrap();

        let buf = unsafe {
            let ptr = NonNull::new_unchecked(buf_alloc.as_mut_ptr());
            let len = buf_alloc.capacity();
            CircularBuf::from_raw_parts(ptr, len)
        };
        let pending_io = None;
        let end_of_file = false;

        #[cfg(not(feature = "sgx"))]
        let msg_param: *mut MsgParam = Box::into_raw(Box::new(unsafe { std::mem::zeroed() }));

        #[cfg(feature = "sgx")]
        let msg_param_alloc = UntrustedAllocator::new(core::mem::size_of::<MsgParam>(), 8).unwrap();
        #[cfg(feature = "sgx")]
        let msg_param: *mut MsgParam = msg_param_alloc.as_mut_ptr() as _;

        Inner {
            buf: ManuallyDrop::new(buf),
            buf_alloc: ManuallyDrop::new(buf_alloc),
            pending_io,
            end_of_file,
            msg_param: ManuallyDrop::new(msg_param),
            #[cfg(feature = "sgx")]
            msg_param_alloc: ManuallyDrop::new(msg_param_alloc),
        }
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        // When the receiver is dropped, all pending async I/O should have been completed.
        debug_assert!(self.pending_io.is_none());

        // Since buf uses the memory allocated from buf_alloc, we must first drop buf,
        // then buf_alloc.
        unsafe {
            ManuallyDrop::drop(&mut self.buf);
            ManuallyDrop::drop(&mut self.buf_alloc);

            #[cfg(not(feature = "sgx"))]
            drop(Box::from_raw(*self.msg_param));

            ManuallyDrop::drop(&mut self.msg_param);

            #[cfg(feature = "sgx")]
            ManuallyDrop::drop(&mut self.msg_param_alloc);
        }
    }
}
