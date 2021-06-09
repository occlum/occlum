//! A more user-friendly io_uring crate.
//!
//! # Overview
//!
//! While the original [io_uring crate](https://github.com/tokio-rs/io-uring) exposes io_uring's API in Rust, it has
//! one big shortcoming: users have to manually pop entries out of the completion queue and map those entries to
//! user requests. It makes the APIs cumbersome to use.
//!
//! This crate provides more user-friend APIs with the following features:
//!
//! * Callback-based. On the completion of an I/O request, the corresponding user-registered
//! callback will get invoked. No manual dispatching of I/O completions.
//!
//! * Async/await-ready. After submitting an I/O request, the user will get a handle that
//! represents the on-going I/O request. The user can await the handle (as it is a `Future`).
//!
//! * Polling-based I/O. Both I/O submissions and completions can be easily done in polling mode.
//!
//! # Usage
//!
//! ## Construct an io_uring instance
//!
//!
//! ## Submit I/O requests
//!
//!
//! ## Poll I/O completions
//!
//!
//! # Handles
//!
//! ## The contract
//!
//! ## I/O cancelling

#![feature(get_mut_unchecked)]
#![cfg_attr(feature = "sgx", no_std)]

#[cfg(feature = "sgx")]
extern crate sgx_types;
#[cfg(feature = "sgx")]
#[macro_use]
extern crate sgx_tstd as std;
#[cfg(feature = "sgx")]
extern crate sgx_libc as libc;
#[cfg(feature = "sgx")]
extern crate sgx_trts;

extern crate atomic;
extern crate io_uring;
extern crate slab;

#[cfg(feature = "sgx")]
use std::prelude::v1::*;

use std::io;
use std::sync::Arc;
#[cfg(not(feature = "sgx"))]
use std::sync::Mutex;
#[cfg(feature = "sgx")]
use std::sync::SgxMutex as Mutex;

use io_uring::opcode::{self, types};
use io_uring::squeue::Entry as SqEntry;
use slab::Slab;

use crate::io_handle::IoToken;

mod io_handle;

pub use crate::io_handle::{IoHandle, IoState};
pub use io_uring::opcode::types::{Fd, Fixed};

/// An io_uring instance augmented with callback-based I/O interfaces.
///
/// # Safety
///
/// All I/O methods are based on the assumption that the resources (e.g., file descriptors, pointers, etc.)
/// given in their arguments are valid before the completion of the async I/O.
pub struct IoUring {
    ring: io_uring::concurrent::IoUring,
    token_table: Mutex<Slab<Arc<IoToken>>>,
}

impl Drop for IoUring {
    fn drop(&mut self) {
        // By the end of the life of the io_uring instance, its token table should have been emptied.
        // This emptyness check prevents handles created by this io_uring become "dangling".
        // That is, no user will ever hold a handle whose associated io_uring instance has
        // been destroyed.
        let token_table = self.token_table.lock().unwrap();
        assert!(token_table.len() == 0);
    }
}

impl IoUring {
    /// Internal constructor.
    ///
    /// Users should use `Builder` instead.
    pub(crate) fn new(ring: io_uring::IoUring) -> Self {
        let ring = ring.concurrent();
        let token_table = Mutex::new(Slab::new());
        Self { ring, token_table }
    }

    /// Push an accept request into the submission queue of the io_uring.
    ///
    /// # Safety
    ///
    /// See the safety section of the `IoUring`.
    pub unsafe fn accept(
        &self,
        fd: Fd,
        // fixed_fd: Fixed,
        addr: *mut libc::sockaddr,
        addrlen: *mut libc::socklen_t,
        flags: u32,
        callback: impl FnOnce(i32) + Send + 'static,
    ) -> IoHandle {
        let entry = opcode::Accept::new(fd, addr, addrlen).flags(flags).build();
        self.push_entry(entry, callback)
    }

    /// Push a connect request into the submission queue of the io_uring.
    ///
    /// # Safety
    ///
    /// See the safety section of the `IoUring`.
    pub unsafe fn connect(
        &self,
        fd: Fd,
        // fixed_fd: Fixed,
        addr: *const libc::sockaddr,
        addrlen: libc::socklen_t,
        callback: impl FnOnce(i32) + Send + 'static,
    ) -> IoHandle {
        let entry = opcode::Connect::new(fd, addr, addrlen).build();
        self.push_entry(entry, callback)
    }

    /// Push a poll_add request into the submission queue of the io_uring.
    ///
    /// # Safety
    ///
    /// See the safety section of the `IoUring`.
    pub unsafe fn poll_add(
        &self,
        fd: Fd,
        // fixed_fd: Fixed,
        flags: u32,
        callback: impl FnOnce(i32) + Send + 'static,
    ) -> IoHandle {
        let entry = opcode::PollAdd::new(fd, flags).build();
        self.push_entry(entry, callback)
    }

    /// Push a poll_remove request into the submission queue of the io_uring.
    ///
    /// # Safety
    ///
    /// See the safety section of the `IoUring`.
    pub unsafe fn poll_remove(
        &self,
        user_data: u64,
        callback: impl FnOnce(i32) + Send + 'static,
    ) -> IoHandle {
        let entry = opcode::PollRemove::new(user_data).build();
        self.push_entry(entry, callback)
    }

    /// Push a read request into the submission queue of the io_uring.
    ///
    /// # Safety
    ///
    /// See the safety section of the `IoUring`.
    pub unsafe fn read(
        &self,
        fd: Fd,
        // fixed_fd: Fixed,
        buf: *mut u8,
        len: u32,
        offset: libc::off_t,
        flags: types::RwFlags,
        callback: impl FnOnce(i32) + Send + 'static,
    ) -> IoHandle {
        let entry = opcode::Read::new(fd, buf, len)
            .offset(offset)
            .rw_flags(flags)
            .build();
        self.push_entry(entry, callback)
    }

    /// Push a write request into the submission queue of the io_uring.
    ///
    /// # Safety
    ///
    /// See the safety section of the `IoUring`.
    pub unsafe fn write(
        &self,
        fd: Fd,
        // fixed_fd: Fixed,
        buf: *const u8,
        len: u32,
        offset: libc::off_t,
        flags: types::RwFlags,
        callback: impl FnOnce(i32) + Send + 'static,
    ) -> IoHandle {
        let entry = opcode::Write::new(fd, buf, len)
            .offset(offset)
            .rw_flags(flags)
            .build();
        self.push_entry(entry, callback)
    }

    /// Push a readv request into the submission queue of the io_uring.
    ///
    /// # Safety
    ///
    /// See the safety section of the `IoUring`.
    pub unsafe fn readv(
        &self,
        fd: Fd,
        // fixed_fd: Fixed,
        iovec: *const libc::iovec,
        len: u32,
        offset: libc::off_t,
        flags: types::RwFlags,
        callback: impl FnOnce(i32) + Send + 'static,
    ) -> IoHandle {
        let entry = opcode::Readv::new(fd, iovec, len)
            .offset(offset)
            .rw_flags(flags)
            .build();
        self.push_entry(entry, callback)
    }

    /// Push a writev request into the submission queue of the io_uring.
    ///
    /// # Safety
    ///
    /// See the safety section of the `IoUring`.
    pub unsafe fn writev(
        &self,
        fd: Fd,
        // fixed_fd: Fixed,
        iovec: *const libc::iovec,
        len: u32,
        offset: libc::off_t,
        flags: types::RwFlags,
        callback: impl FnOnce(i32) + Send + 'static,
    ) -> IoHandle {
        let entry = opcode::Writev::new(fd, iovec, len)
            .offset(offset)
            .rw_flags(flags)
            .build();
        self.push_entry(entry, callback)
    }

    /// Push a recvmsg request into the submission queue of the io_uring.
    ///
    /// # Safety
    ///
    /// See the safety section of the `IoUring`.
    pub unsafe fn recvmsg(
        &self,
        fd: Fd,
        // fixed_fd: Fixed,
        msg: *mut libc::msghdr,
        flags: u32,
        callback: impl FnOnce(i32) + Send + 'static,
    ) -> IoHandle {
        let entry = opcode::RecvMsg::new(fd, msg).flags(flags).build();
        self.push_entry(entry, callback)
    }

    /// Push a sendmsg request into the submission queue of the io_uring.
    ///
    /// # Safety
    ///
    /// See the safety section of the `IoUring`.
    pub unsafe fn sendmsg(
        &self,
        fd: Fd,
        // fixed_fd: Fixed,
        msg: *const libc::msghdr,
        flags: u32,
        callback: impl FnOnce(i32) + Send + 'static,
    ) -> IoHandle {
        let entry = opcode::SendMsg::new(fd, msg).flags(flags).build();
        self.push_entry(entry, callback)
    }

    /// Push a fsync request into the submission queue of the io_uring.
    ///
    /// # Safety
    ///
    /// See the safety section of the `IoUring`.
    pub unsafe fn fsync(
        &self,
        fd: Fd,
        datasync: bool,
        callback: impl FnOnce(i32) + Send + 'static,
    ) -> IoHandle {
        let entry = if datasync {
            opcode::Fsync::new(fd)
                .flags(types::FsyncFlags::DATASYNC)
                .build()
        } else {
            opcode::Fsync::new(fd).build()
        };
        self.push_entry(entry, callback)
    }

    /// Submit all I/O requests in the submission queue of io_uring.
    ///
    /// Without calling this method, new I/O requests pushed into the submission queue will
    /// not get popped by Linux kernel.
    pub fn submit_requests(&self) {
        if let Err(e) = self.ring.submit() {
            panic!("submit failed, error: {}", e);
        }
    }

    /// Poll new I/O completions in the completions queue of io_uring.
    ///
    /// Upon receiving completed I/O, the corresponding user-registered callback functions
    /// will get invoked and the `IoHandle` (as a `Future`) will become ready.
    pub fn poll_completions(&self) {
        let cq = self.ring.completion();
        while let Some(cqe) = cq.pop() {
            let retval = cqe.result();
            let io_token = {
                let token_idx = cqe.user_data() as usize;
                let mut token_table = self.token_table.lock().unwrap();
                token_table.remove(token_idx)
            };
            io_token.complete(retval);
        }
    }

    /// Start a helper thread that is busy doing `io_uring_enter` on this io_uring instance.
    ///
    /// # Why a helper thread?
    ///
    /// The io_uring implementation on the latest Linux kernel only has a limited (even buggy)
    /// support for kernel-polling mode. To address this limitation, we simulate the kernel-polling
    /// mode in the user space by starting a helper thread that keeps entering into the kernel
    /// and polling I/O requests from the submission queue of the io_uring instance.
    ///
    /// While the helper thread comes with a performance cost, we believe it is acceptable as a
    /// short-term workaround. We expect the io_uring implementation in Linux kernel to become mature
    /// in the near future.
    ///
    /// # Safety
    ///
    /// This API is _unsafe_ due to the fact that the thread has no idea when the io_uring instance
    /// is destroyed, thus invalidating the file descriptor of io_uring that is still in use by the thread.
    /// This unexpected invalidation is---in most cases---harmless. This is because an io_uring
    /// instance is most likely used as a singleton in a process and will not get destroyed until
    /// the end of the process.
    pub unsafe fn start_enter_syscall_thread(&self) {
        self.ring.start_enter_syscall_thread();
    }

    // Push a submission entry to io_uring and return a corresponding handle.
    //
    // Safety. All resources referenced by the entry must be valid before its completion.
    unsafe fn push_entry(
        &self,
        mut entry: SqEntry,
        callback: impl FnOnce(i32) + Send + 'static,
    ) -> IoHandle {
        // Create the user-visible handle that is associated with the submission entry
        let io_handle = {
            let mut token_table = self.token_table.lock().unwrap();
            let token_slot = token_table.vacant_entry();
            let token_key = token_slot.key() as u64;
            let token = Arc::new(IoToken::new(callback));
            token_slot.insert(token.clone());
            let handle = IoHandle::new(token);

            // Associated entry with token, the latter of which is pointed to by handle.
            entry = entry.user_data(token_key);

            handle
        };

        if self.ring.submission().push(entry).is_err() {
            panic!("sq must be large enough");
        }

        io_handle
    }

    /// Cancel all ongoing async I/O.
    pub fn cancel_all(&self) {
        todo!("implement cancel in the future");
    }
}

/// A builder for `IoUring`.
#[derive(Default)]
pub struct Builder {
    inner: io_uring::Builder,
}

impl Builder {
    /// Creates a `IoUring` builder.
    pub fn new() -> Self {
        Default::default()
    }

    /// When this flag is specified, a kernel thread is created to perform submission queue polling.
    /// An io_uring instance configured in this way enables an application to issue I/O
    /// without ever context switching into the kernel.
    pub fn setup_sqpoll(&mut self, idle: impl Into<Option<u32>>) -> &mut Self {
        self.inner.setup_sqpoll(idle);
        self
    }

    /// If this flag is specified,
    /// then the poll thread will be bound to the cpu set in the value.
    /// This flag is only meaningful when [Builder::setup_sqpoll] is enabled.
    pub fn setup_sqpoll_cpu(&mut self, n: u32) -> &mut Self {
        self.inner.setup_sqpoll_cpu(n);
        self
    }

    /// Create the completion queue with struct `io_uring_params.cq_entries` entries.
    /// The value must be greater than entries, and may be rounded up to the next power-of-two.
    pub fn setup_cqsize(&mut self, n: u32) -> &mut Self {
        self.inner.setup_cqsize(n);
        self
    }

    /// Build a [IoUring].
    #[inline]
    pub fn build(&self, entries: u32) -> io::Result<IoUring> {
        let io_uring_inner = self.inner.build(entries)?;
        let io_uring = IoUring::new(io_uring_inner);
        Ok(io_uring)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{IoSlice, IoSliceMut};
    use std::os::unix::io::AsRawFd;
    use std::sync::{Arc, Mutex};

    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }

    #[test]
    fn test_builder() {
        let _io_uring = Builder::new().setup_sqpoll(1000).build(256).unwrap();
    }

    #[test]
    fn test_new() {
        let _io_uring = IoUring::new(io_uring::IoUring::new(256).unwrap());
    }

    #[test]
    fn test_writev_readv() {
        let io_uring = IoUring::new(io_uring::IoUring::new(256).unwrap());

        let fd = tempfile::tempfile().unwrap();
        let fd = Fd(fd.as_raw_fd());

        let text = b"1234";
        let text2 = b"5678";
        let mut output = vec![0; text.len()];
        let mut output2 = vec![0; text2.len()];

        let w_iovecs = vec![IoSlice::new(text), IoSlice::new(text2)];
        let r_iovecs = vec![IoSliceMut::new(&mut output), IoSliceMut::new(&mut output2)];

        let complete_io: Arc<Mutex<Option<i32>>> = Arc::new(Mutex::new(None));

        let clone = complete_io.clone();
        let complete_fn = move |retval: i32| {
            let mut inner = clone.lock().unwrap();
            inner.replace(retval);
        };
        let _handle = unsafe {
            io_uring.writev(
                fd,
                w_iovecs.as_ptr().cast(),
                w_iovecs.len() as _,
                0,
                0,
                complete_fn,
            )
        };
        io_uring.submit_requests();
        loop {
            io_uring.poll_completions();

            let clone = complete_io.clone();
            let mut inner = clone.lock().unwrap();
            if inner.is_some() {
                let retval = inner.take().unwrap();
                assert_eq!(retval, (text.len() + text2.len()) as i32);
                break;
            }
        }

        let clone = complete_io.clone();
        let complete_fn = move |retval: i32| {
            let mut inner = clone.lock().unwrap();
            inner.replace(retval);
        };
        let _handle = unsafe {
            io_uring.readv(
                fd,
                r_iovecs.as_ptr().cast(),
                r_iovecs.len() as _,
                0,
                0,
                complete_fn,
            )
        };
        io_uring.submit_requests();
        loop {
            io_uring.poll_completions();

            let clone = complete_io.clone();
            let mut inner = clone.lock().unwrap();
            if inner.is_some() {
                let retval = inner.take().unwrap();
                assert_eq!(retval, (text.len() + text2.len()) as i32);
                assert_eq!(&output, text);
                assert_eq!(&output2, text2);
                break;
            }
        }
    }
}
