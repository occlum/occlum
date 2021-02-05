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

use crate::io::receiver::MsgParam;
use crate::io::{Common, IoUringProvider};
use crate::poll::{Events, Poller};
use crate::util::CircularBuf;

pub struct Sender<P: IoUringProvider> {
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
    is_shutdown: bool,
    msg_param: ManuallyDrop<*mut MsgParam>,
    #[cfg(feature = "sgx")]
    msg_param_alloc: ManuallyDrop<UntrustedAllocator>,
}

unsafe impl Send for Inner {}

impl<P: IoUringProvider> Sender<P> {
    /// Construct the sender of a socket.
    pub(crate) fn new(common: Arc<Common<P>>, buf_size: usize) -> Arc<Self> {
        let inner = Mutex::new(Inner::new(buf_size));
        let new_self = Arc::new(Self { common, inner });
        new_self
    }

    pub async fn write(self: &Arc<Self>, buf: &[u8]) -> i32 {
        // Initialize the poller only when needed
        let mut poller = None;
        loop {
            // Attempt to write
            let ret = self.try_write(buf);
            // If the sender is not writable for now, poll again
            if ret != -libc::EAGAIN {
                return ret;
            }

            // Wait for interesting events by polling
            if poller.is_none() {
                poller = Some(Poller::new());
            }
            let mask = Events::OUT;
            let events = self.common.pollee().poll_by(mask, poller.as_mut());
            if events.is_empty() {
                poller.as_ref().unwrap().wait().await;
            }
        }
    }

    fn try_write(self: &Arc<Self>, buf: &[u8]) -> i32 {
        let mut inner = self.inner.lock().unwrap();

        if inner.is_shutdown {
            return -libc::EPIPE;
        }
        if let Some(error) = self.common.error() {
            return error;
        }
        if buf.len() == 0 {
            return 0;
        }

        let nbytes = inner.buf.produce(buf);

        if inner.buf.is_full() {
            // Mark the socket as non-writable
            self.common.pollee().remove(Events::OUT);
        }

        if nbytes == 0 {
            return -libc::EAGAIN;
        }

        if inner.pending_io.is_none() {
            self.flush_buf(&mut inner);
        }

        nbytes as i32
    }

    fn flush_buf(self: &Arc<Self>, inner: &mut MutexGuard<Inner>) {
        debug_assert!(!inner.buf.is_empty());
        debug_assert!(inner.pending_io.is_none());

        // Init the callback invoked upon the completion of the async flush
        let sender = self.clone();
        let complete_fn = move |retval: i32| {
            let mut inner = sender.inner.lock().unwrap();

            // Release the handle to the async fill
            inner.pending_io.take();

            // Handle the two cases of success and error
            if retval >= 0 {
                let nbytes = retval as usize;
                inner.buf.consume_without_copy(nbytes);
                if !inner.is_shutdown {
                    sender.common.pollee().add(Events::OUT);
                }
            } else {
                // Discard all data in the send buf
                let consumable_bytes = inner.buf.consumable();
                inner.buf.consume_without_copy(consumable_bytes);

                sender.common.set_error(retval);
                sender.common.pollee().add(Events::ERR);
            }

            // Flush there are remaining data in the buf
            if !inner.buf.is_empty() {
                sender.flush_buf(&mut inner);
            } else {
                if inner.is_shutdown {
                    unsafe {
                        #[cfg(not(feature = "sgx"))]
                        libc::shutdown(sender.common.fd(), libc::SHUT_WR);
                        #[cfg(feature = "sgx")]
                        libc::ocall::shutdown(sender.common.fd(), libc::SHUT_WR);
                    }
                }
            }
        };

        // Construct the iovec for the async flush
        let mut iovec_len = 1;
        let msg_param_ptr = *inner.msg_param;
        let mut msg_ptr = unsafe { &mut (*msg_param_ptr).msg as *mut libc::msghdr };
        unsafe {
            inner.buf.with_consumer_view(|part0, part1| {
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

                // Only access the consumer's data; zero bytes consumed for now.
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
        let handle = unsafe { io_uring.sendmsg(Fd(self.common.fd()), msg_ptr, 0, complete_fn) };
        inner.pending_io.replace(handle);
    }

    /// Shutdown the sender.
    ///
    /// After shutdowning, the sender will no longer be able to write more data,
    /// only flushing the already-written data to the underlying io_uring.
    pub fn shutdown(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.is_shutdown = true;

        if inner.pending_io.is_none() && inner.buf.is_empty() {
            unsafe {
                #[cfg(not(feature = "sgx"))]
                libc::shutdown(self.common.fd(), libc::SHUT_WR);
                #[cfg(feature = "sgx")]
                libc::ocall::shutdown(self.common.fd(), libc::SHUT_WR);
            }
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
        let is_shutdown = false;

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
            is_shutdown,
            msg_param: ManuallyDrop::new(msg_param),
            #[cfg(feature = "sgx")]
            msg_param_alloc: ManuallyDrop::new(msg_param_alloc),
        }
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        // When the sender is dropped, all pending async I/O should have been completed.
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
