use std::collections::VecDeque;
use std::mem::ManuallyDrop;
#[cfg(feature = "sgx")]
use std::prelude::v1::*;
#[cfg(not(feature = "sgx"))]
use std::sync::{Arc, Mutex, MutexGuard};
#[cfg(feature = "sgx")]
use std::sync::{Arc, SgxMutex as Mutex, SgxMutexGuard as MutexGuard};

use io_uring_callback::{Fd, Handle};
#[cfg(feature = "sgx")]
use sgx_untrusted_alloc::UntrustedAllocator;
use slab::Slab;

use crate::io::{Common, IoUringProvider};
use crate::poll::{Events, Poller};
use crate::util::RawSlab;

pub struct Acceptor<P: IoUringProvider> {
    common: Arc<Common<P>>,
    inner: Mutex<Inner>,
}

struct Inner {
    accept_slab: Slab<Accept>,
    param_raw_slab: ManuallyDrop<RawSlab<AcceptParam>>,
    // The underlying heap buffer for param_slab.
    #[cfg(not(feature = "sgx"))]
    param_raw_slab_buf: ManuallyDrop<Vec<AcceptParam>>,
    #[cfg(feature = "sgx")]
    param_raw_slab_buf: ManuallyDrop<UntrustedAllocator>,
    completed_indexes: VecDeque<usize>,
}

unsafe impl Send for Inner {}

struct AcceptParam {
    addr: libc::sockaddr_in,
    addrlen: libc::socklen_t,
}

enum Accept {
    Pending {
        param: *mut AcceptParam,
        handle: Handle,
    },
    Completed {
        param: *mut AcceptParam,
        fd: i32,
    },
}

// Implementation for Acceptor

impl<P: IoUringProvider> Acceptor<P> {
    pub(crate) fn new(backlog: usize, common: Arc<Common<P>>) -> Arc<Self> {
        let inner = Mutex::new(Inner::new(backlog));
        let new_self = Arc::new(Self { common, inner });

        {
            let mut inner = new_self.inner.lock().unwrap();
            new_self.initiate_async_accepts(&mut inner);
        }

        new_self
    }

    pub async fn accept(self: &Arc<Self>, mut output_addr: Option<&mut libc::sockaddr_in>) -> i32 {
        // Init the poller only when needed
        let mut poller = None;
        loop {
            // Attempt to accept
            let ret = self.try_accept(&mut output_addr);
            if ret != -libc::EAGAIN {
                return ret;
            }

            // Ensure the poller is initialized
            if poller.is_none() {
                poller = Some(Poller::new());
            }
            // Wait for interesting events by polling
            let mask = Events::IN;
            let events = self.common.pollee().poll_by(mask, poller.as_mut());
            if events.is_empty() {
                poller.as_ref().unwrap().wait().await;
            }
        }
    }

    fn try_accept(self: &Arc<Self>, output_addr: &mut Option<&mut libc::sockaddr_in>) -> i32 {
        let mut inner = self.inner.lock().unwrap();

        // Try to return an already-completed accept operation
        let completed_index = match inner.completed_indexes.pop_front() {
            None => {
                if let Some(error) = self.common.error() {
                    return error;
                }
                return -libc::EAGAIN;
            }
            Some(completed_index) => completed_index,
        };

        if inner.completed_indexes.is_empty() {
            // Mark the socket not ready to accept new incoming sockets
            self.common.pollee().remove(Events::IN);
        }

        let completed = inner.accept_slab.get(completed_index).unwrap();
        match completed {
            Accept::Completed { param, fd } => {
                let param = *param;
                let addr = unsafe { (*param).addr_mut_ptr() };
                let fd = *fd;
                drop(completed);

                if let Some(output_addr) = output_addr.as_mut() {
                    **output_addr = unsafe { *addr };
                }

                // Free the resources associated with the completed accept
                unsafe { inner.param_raw_slab.dealloc(param) };
                inner.accept_slab.remove(completed_index);

                self.initiate_async_accepts(&mut inner);
                return fd;
            }
            Accept::Pending { .. } => unreachable!("must have been completed"),
        }
    }

    fn initiate_async_accepts(self: &Arc<Self>, inner: &mut MutexGuard<Inner>) {
        // We hold the following invariant:
        //
        //      The length of backlog >= # of pending accepts + # of completed accepts
        //
        // And for the maximal performance, we try to make the two sides equal.
        while inner.accept_slab.len() < inner.accept_slab.capacity() {
            // Allocate resources for the new accept from the slabs
            let param = inner.param_raw_slab.alloc().unwrap();
            let addr = unsafe { (*param).addr_mut_ptr() };
            let addrlen = unsafe { (*param).addrlen_mut_ptr() };
            unsafe {
                *addrlen = std::mem::size_of::<libc::sockaddr_in>() as u32;
            }
            let accept_slab_entry = inner.accept_slab.vacant_entry();

            // Prepare the arguments for the io_uring accept
            let flags = 0;
            let callback = {
                let accept_slab_index = accept_slab_entry.key();
                let acceptor = self.clone();
                move |retval: i32| {
                    let mut inner = acceptor.inner.lock().unwrap();
                    let pending_accept = inner.accept_slab.get_mut(accept_slab_index).unwrap();

                    if retval < 0 {
                        acceptor.common.set_error(retval);
                        acceptor.common.pollee().add(Events::ERR);

                        // Free the resources allocated from the slabs
                        let param = pending_accept.param();
                        drop(pending_accept);
                        unsafe { inner.param_raw_slab.dealloc(param) };
                        inner.accept_slab.remove(accept_slab_index);

                        return;
                    }

                    let fd = retval;
                    pending_accept.complete(fd);
                    inner.completed_indexes.push_back(accept_slab_index);

                    acceptor.common.pollee().add(Events::IN);
                }
            };
            let io_uring = self.common.io_uring();
            let handle = unsafe {
                io_uring.accept(
                    Fd(self.common.fd()),
                    addr as *mut libc::sockaddr,
                    addrlen,
                    flags,
                    callback,
                )
            };

            // Record the pending accept
            let pending_accept = Accept::Pending { param, handle };
            accept_slab_entry.insert(pending_accept);
        }
    }
}

// Implementation for Inner

impl Inner {
    pub fn new(backlog: usize) -> Self {
        let backlog = {
            const MIN_BACKLOG: usize = 1;
            const MAX_BACKLOG: usize = 16;
            backlog.max(MIN_BACKLOG).min(MAX_BACKLOG)
        };

        let accept_slab = Slab::with_capacity(backlog);

        #[cfg(not(feature = "sgx"))]
        let mut param_raw_slab_buf = ManuallyDrop::new(Vec::with_capacity(backlog));
        #[cfg(feature = "sgx")]
        let param_raw_slab_buf = ManuallyDrop::new(
            UntrustedAllocator::new(backlog * core::mem::size_of::<AcceptParam>(), 8).unwrap(),
        );
        let param_raw_slab = unsafe {
            let ptr = param_raw_slab_buf.as_mut_ptr() as *mut AcceptParam;
            ManuallyDrop::new(RawSlab::new(ptr, backlog))
        };

        let completed_indexes = VecDeque::with_capacity(backlog);
        Self {
            accept_slab,
            param_raw_slab,
            param_raw_slab_buf,
            completed_indexes,
        }
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        // Free all resources associated with the completed accept
        for completed_index in self.completed_indexes.drain(..) {
            let completed_accept = self.accept_slab.get(completed_index).unwrap();

            let param = completed_accept.param();
            unsafe {
                self.param_raw_slab.dealloc(param);
            }

            let fd = completed_accept.fd().unwrap();
            unsafe {
                #[cfg(not(feature = "sgx"))]
                libc::close(fd);
                #[cfg(feature = "sgx")]
                libc::ocall::close(fd);
            }

            self.accept_slab.remove(completed_index);
        }

        // Since all pending accepts should have completed and all completed
        // accepts are freed, the slab should be empty.
        debug_assert!(self.accept_slab.is_empty());
        // So should the addr slab
        debug_assert!(self.param_raw_slab.allocated() == 0);

        // Since addr_raw_slab uses the memory allocated from addr_raw_slab_buf, we must
        // first drop the Vec object, then the Slab object.
        unsafe {
            ManuallyDrop::drop(&mut self.param_raw_slab);
            ManuallyDrop::drop(&mut self.param_raw_slab_buf);
        }
    }
}

// Implementation for Accept

impl Accept {
    pub fn param(&self) -> *mut AcceptParam {
        match *self {
            Self::Pending { param, .. } => param,
            Self::Completed { param, .. } => param,
        }
    }

    pub fn complete(&mut self, fd: i32) {
        *self = match self {
            Self::Completed { .. } => {
                panic!("a completed accept cannot be complete again");
            }
            Self::Pending { param, handle } => Self::Completed { param: *param, fd },
        }
    }

    pub fn fd(&self) -> Option<i32> {
        match *self {
            Self::Completed { fd, .. } => Some(fd),
            Self::Pending { .. } => None,
        }
    }
}

impl AcceptParam {
    pub fn addr_mut_ptr(&mut self) -> *mut libc::sockaddr_in {
        &mut self.addr as _
    }

    pub fn addrlen_mut_ptr(&mut self) -> *mut libc::socklen_t {
        &mut self.addrlen as _
    }
}
