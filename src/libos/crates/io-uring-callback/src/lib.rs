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
//! Use [`Builder`] to create a new instance of [`IoUring`].
//!
//! ```
//! use io_uring_callback::{Builder, IoUring};
//!
//! let io_uring: IoUring = Builder::new().build(256).unwrap();
//! ```
//!
//! A number of I/O operations are supported, e.g., `read`, `write`, `fsync`, `sendmsg`,
//! `recvmsg`, etc. Requests for such I/O operations can be pushed into the submission
//! queue of the io_uring with the corresponding methods.
//!
//! ```
//! # use io_uring_callback::{Builder};
//! use io_uring_callback::{Fd, RwFlags};
//!
//! # let io_uring = Builder::new().build(256).unwrap();
//! let fd = Fd(1); // use the stdout
//! let msg = "hello world\0";
//! let completion_callback = move |retval: i32| {
//!     assert!(retval > 0);
//! };
//! let handle = unsafe {
//!     io_uring.write(fd, msg.as_ptr(), msg.len() as u32, 0, RwFlags::default(), completion_callback)
//! };
//! # io_uring.submit_requests();
//! # while handle.retval().is_none() {
//! #    io_uring.wait_completions(1);
//! # }
//! ```
//!
//! You have to two ways to get notified about the completion of I/O requests. The first
//! is through the registered callback function and the second is by `await`ing the handle
//! (which is a `Future`) obtained as a result of pushing I/O requests.
//!
//! After pushing a batch of I/O requests into the submission queue, you can now _submit_ them
//! to the Linux kernel. Without an explict submit, Linux will not be aware of the new I/O requests.
//! ```
//! # use io_uring_callback::{Builder};
//! # let io_uring = Builder::new().build(256).unwrap();
//! io_uring.submit_requests();
//! ```
//!
//! After completing the I/O requests, Linux will push I/O responses into the completion queue of
//! the io_uring instance. You need _periodically_ poll completions from the queue:
//! ```no_run
//! # use io_uring_callback::{Builder};
//! # let io_uring = Builder::new().build(256).unwrap();
//! let min_complete = 1;
//! let polling_retries = 5000;
//! io_uring.poll_completions(min_complete, polling_retries);
//! ```
//! which will trigger registered callbacks and wake up handles.
//!
//! When the I/O request is completed (the request is processed or cancelled by the kernel),
//! `poll_completions` will trigger the user-registered callback.
//!
//! # I/O Handles
//!
//! After submitting an I/O request, the user will get as the return value
//! an instance of [`IoHandle`], which represents the submitted I/O requests.
//!
//! So why bother keeping I/O handles? The reasons are three-fold.
//!
//! - First, as a future, `IoHandle` allows you to await on it, which is quite
//! convenient if you happen to use io_uring with Rust's async/await.
//! - Second, `IoHandle` makes it possible to _cancel_ on-going I/O requests.
//! - Third, it makes the whole APIs less prone to memory safety issues. Recall that all I/O submitting
//! methods (e.g., `write`, `accept`, etc.) are _unsafe_ as there are no guarantee that
//! their arguments---like FDs or buffer pointers---are valid throughout the lifetime of
//! an I/O request. What if an user accidentally releases the in-use resources associated with
//! an on-going I/O request? I/O handles can detect such programming bugs as long as
//! the handles are also released along with other in-use I/O resources (which is most likely).
//! This is because when an `IoHandle` is dropped, we will panic if its state is neither
//! processed (`IoState::Processed`) or canceled (`IoState::Canceled`). That is, dropping
//! an `IoHandle` that is still in-use is forbidden.
//!
//! After pushing an I/O request into the submission queue, you will get an `IoHandle`.
//! With this handle, you can cancel the I/O request.
//! ```
//! # use io_uring_callback::Builder;
//! use io_uring_callback::{Timespec, TimeoutFlags};
//!
//! # let io_uring = Builder::new().build(256).unwrap();
//! let tp = Timespec { tv_sec: 1, tv_nsec: 0, };
//! let completion_callback = move |_retval: i32| {};
//! let handle = unsafe {
//!     io_uring.timeout(&tp as *const _, 0, TimeoutFlags::empty(), completion_callback)
//! };
//! io_uring.submit_requests();
//! unsafe { io_uring.cancel(&handle); }
//! io_uring.wait_completions(1);
//! ```

#![feature(get_mut_unchecked)]
#![cfg_attr(feature = "sgx", no_std)]

#[cfg(feature = "sgx")]
extern crate sgx_libc as libc;
#[cfg(feature = "sgx")]
extern crate sgx_tstd as std;

use std::io;
use std::sync::Arc;
cfg_if::cfg_if! {
    if #[cfg(feature = "sgx")] {
        use std::prelude::v1::*;
        use std::sync::SgxMutex as Mutex;
    } else {
        use std::sync::Mutex;
    }
}

use io_uring::opcode::{self, types};
use io_uring::squeue::Entry as SqEntry;
use slab::Slab;

use crate::io_handle::IoToken;

mod io_handle;

pub use crate::io_handle::{IoHandle, IoState};
pub use io_uring::opcode::types::{Fd, RwFlags, TimeoutFlags, Timespec};

/// An io_uring instance.
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
    /// The magic token_key for Cancel I/O request.
    /// The magic token_key should be different from the token_table's keys.
    const CANCEL_TOKEN_KEY: u64 = u64::MAX;

    /// Constructor for internal uses.
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
        addr: *const libc::sockaddr,
        addrlen: libc::socklen_t,
        callback: impl FnOnce(i32) + Send + 'static,
    ) -> IoHandle {
        let entry = opcode::Connect::new(fd, addr, addrlen).build();
        self.push_entry(entry, callback)
    }

    /// Push a poll request into the submission queue of the io_uring.
    ///
    /// # Safety
    ///
    /// See the safety section of the `IoUring`.
    pub unsafe fn poll(
        &self,
        fd: Fd,
        flags: u32,
        callback: impl FnOnce(i32) + Send + 'static,
    ) -> IoHandle {
        let entry = opcode::PollAdd::new(fd, flags).build();
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

    /// Push a timeout request into the submission queue of the io_uring.
    ///
    /// # Safety
    ///
    /// See the safety section of the `IoUring`.
    pub unsafe fn timeout(
        &self,
        timespec: *const types::Timespec,
        count: u32,
        flags: types::TimeoutFlags,
        callback: impl FnOnce(i32) + Send + 'static,
    ) -> IoHandle {
        let entry = opcode::Timeout::new(timespec)
            .count(count)
            .flags(flags)
            .build();
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

    /// Poll new I/O completions in the completions queue of io_uring
    /// and return the number of I/O completions.
    ///
    /// Upon receiving completed I/O, the corresponding user-registered callback functions
    /// will get invoked and the `IoHandle` (as a `Future`) will become ready.
    ///
    /// The method guarantees at least the specified number of entries are
    /// popped from the completion queue. To do so, it starts by polling the
    /// completion queue for at most the specified number of retries.
    /// If the number of completion entries popped so far does not reach the
    /// the specified minimum value, then the method shall block
    /// until new completions arrive. After getting unblocked, the method
    /// repeats polling.
    ///
    /// If the user does not want to the method to block, set `min_complete`
    /// to 0. If the user does not want to the method to busy polling, set
    /// `polling_retries` to 0.
    pub fn poll_completions(&self, min_complete: usize, polling_retries: usize) -> usize {
        let cq = self.ring.completion();
        let mut nr_complete = 0;
        loop {
            // Polling for at most a specified number of times
            let mut nr_retries = 0;
            while nr_retries <= polling_retries {
                if let Some(cqe) = cq.pop() {
                    let retval = cqe.result();
                    let token_key = cqe.user_data();
                    if token_key != IoUring::CANCEL_TOKEN_KEY {
                        let io_token = {
                            let token_idx = token_key as usize;
                            let mut token_table = self.token_table.lock().unwrap();
                            token_table.remove(token_idx)
                        };

                        io_token.complete(retval);
                        nr_complete += 1;
                    }
                } else {
                    nr_retries += 1;
                    std::hint::spin_loop();
                }
            }

            if nr_complete >= min_complete {
                return nr_complete;
            }

            // Wait until at least one new completion entry arrives
            let _ = self.ring.submit_and_wait(1);
        }
    }

    /// Wait for at least the specified number of I/O completions.
    pub fn wait_completions(&self, min_complete: usize) -> usize {
        self.poll_completions(min_complete, 10)
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

    unsafe fn push(&self, entry: SqEntry) {
        loop {
            if self.ring.submission().push(entry.clone()).is_err() {
                if self.ring.enter(1, 1, 0, None).is_err() {
                    panic!("sq broken");
                }
            } else {
                break;
            }
        }
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
            assert!(token_key != IoUring::CANCEL_TOKEN_KEY);

            let token = Arc::new(IoToken::new(callback, token_key));
            token_slot.insert(token.clone());
            let handle = IoHandle::new(token);

            // Associated entry with token, the latter of which is pointed to by handle.
            entry = entry.user_data(token_key);

            handle
        };

        self.push(entry);

        io_handle
    }

    /// Cancel an ongoing I/O request.
    ///
    /// # safety
    ///
    /// The handle must be generated by this IoUring instance.
    pub unsafe fn cancel(&self, handle: &IoHandle) {
        let target_token_key = match handle.0.transit_to_cancelling() {
            Ok(target_token_key) => target_token_key,
            Err(_) => {
                return;
            }
        };
        let mut entry = opcode::AsyncCancel::new(target_token_key).build();
        entry = entry.user_data(IoUring::CANCEL_TOKEN_KEY);

        self.push(entry);
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
    use std::fs::File;
    use std::io::{IoSlice, IoSliceMut, Write};
    use std::os::unix::io::{AsRawFd, FromRawFd};
    use std::thread;
    use std::time::Duration;
    use std::time::Instant;

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

        let complete_fn = move |_retval: i32| {};
        let handle = unsafe {
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

        io_uring.wait_completions(1);
        let retval = handle.retval().unwrap();
        assert_eq!(retval, (text.len() + text2.len()) as i32);

        let complete_fn = move |_retval: i32| {};
        let handle = unsafe {
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

        io_uring.wait_completions(1);
        let retval = handle.retval().unwrap();
        assert_eq!(retval, (text.len() + text2.len()) as i32);
        assert_eq!(&output, text);
        assert_eq!(&output2, text2);
    }

    #[test]
    fn test_poll() {
        let mut fd = unsafe {
            let fd = libc::eventfd(0, libc::EFD_CLOEXEC);
            assert!(fd != -1);
            File::from_raw_fd(fd)
        };

        let io_uring = IoUring::new(io_uring::IoUring::new(256).unwrap());

        let complete_fn = move |_retval: i32| {};
        let handle = unsafe { io_uring.poll(Fd(fd.as_raw_fd()), libc::POLLIN as _, complete_fn) };
        io_uring.submit_requests();

        thread::sleep(Duration::from_millis(100));
        assert_eq!(io_uring.poll_completions(0, 10000), 0);

        fd.write(&0x1u64.to_ne_bytes()).unwrap();
        io_uring.wait_completions(1);
        assert_eq!(handle.retval().unwrap(), 1);
    }

    #[test]
    fn test_cancel_poll() {
        let mut fd = unsafe {
            let fd = libc::eventfd(0, libc::EFD_CLOEXEC);
            assert!(fd != -1);
            File::from_raw_fd(fd)
        };

        let io_uring = IoUring::new(io_uring::IoUring::new(256).unwrap());

        let complete_fn = move |_retval: i32| {};
        let poll_handle =
            unsafe { io_uring.poll(Fd(fd.as_raw_fd()), libc::POLLIN as _, complete_fn) };
        io_uring.submit_requests();

        unsafe {
            io_uring.cancel(&poll_handle);
        }
        io_uring.submit_requests();

        thread::sleep(Duration::from_millis(100));

        fd.write(&0x1u64.to_ne_bytes()).unwrap();
        io_uring.wait_completions(1);

        assert_eq!(poll_handle.retval().unwrap(), -libc::ECANCELED);
    }

    #[test]
    fn test_cancel_poll_failed() {
        let mut fd = unsafe {
            let fd = libc::eventfd(0, libc::EFD_CLOEXEC);
            assert!(fd != -1);
            File::from_raw_fd(fd)
        };

        let io_uring = IoUring::new(io_uring::IoUring::new(256).unwrap());

        let complete_fn = move |_retval: i32| {};
        let poll_handle =
            unsafe { io_uring.poll(Fd(fd.as_raw_fd()), libc::POLLIN as _, complete_fn) };
        io_uring.submit_requests();

        fd.write(&0x1u64.to_ne_bytes()).unwrap();
        io_uring.wait_completions(1);

        unsafe {
            io_uring.cancel(&poll_handle);
        }
        io_uring.submit_requests();

        thread::sleep(Duration::from_millis(100));
        assert_eq!(poll_handle.retval().unwrap(), 1);
    }

    #[test]
    fn test_timeout() {
        let io_uring = IoUring::new(io_uring::IoUring::new(256).unwrap());

        let start = Instant::now();
        let secs = 1;
        let timespec = types::Timespec {
            tv_sec: secs,
            tv_nsec: 0,
        };
        let complete_fn = move |_retval: i32| {};

        let handle = unsafe {
            io_uring.timeout(
                &timespec as *const _,
                0,
                types::TimeoutFlags::empty(),
                complete_fn,
            )
        };
        io_uring.submit_requests();
        io_uring.wait_completions(1);

        assert_eq!(handle.retval().unwrap(), -libc::ETIME);
        assert_eq!(start.elapsed().as_secs(), secs as u64);
    }

    #[test]
    fn test_cancel_timeout() {
        let io_uring = IoUring::new(io_uring::IoUring::new(256).unwrap());

        let start = Instant::now();
        let secs = 1;
        let timespec = types::Timespec {
            tv_sec: secs,
            tv_nsec: 0,
        };

        let complete_fn = move |_retval: i32| {};

        let handle = unsafe {
            io_uring.timeout(
                &timespec as *const _,
                0,
                types::TimeoutFlags::empty(),
                complete_fn,
            )
        };
        io_uring.submit_requests();

        unsafe {
            io_uring.cancel(&handle);
        }
        io_uring.submit_requests();

        io_uring.wait_completions(1);

        assert_eq!(handle.retval().unwrap(), -libc::ECANCELED);
        assert_eq!(start.elapsed().as_secs(), 0);
    }
}
