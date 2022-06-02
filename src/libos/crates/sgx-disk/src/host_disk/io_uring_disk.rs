use block_device::{BioReq, BioSubmission, BioType, BlockDevice};
use fs::File;
use io_uring_callback::{Fd, IoHandle, IoUring};
use new_self_ref_arc::new_self_ref_arc;
use std::io::prelude::*;
use std::marker::PhantomData;
use std::os::unix::io::{AsRawFd, RawFd};
use std::path::{Path, PathBuf};
use std::sync::Weak;

#[cfg(feature = "sgx")]
use sgx_untrusted_alloc::UntrustedBox;

use super::OpenOptions;
use crate::prelude::*;
use crate::HostDisk;

/// Providing an io_uring instance to be used by IoUringDisk.
///
/// This trait is introduced to decouple the creation of io_uring from
/// its users.
pub trait IoUringProvider: Send + Sync + 'static {
    fn io_uring() -> &'static IoUring;
}

/// A type of host disk that implements a block device interface by performing
/// async I/O via Linux's io_uring.
pub struct IoUringDisk<P: IoUringProvider>(Arc<Inner<P>>);

struct Inner<P: IoUringProvider> {
    fd: RawFd,
    file: Mutex<File>,
    path: PathBuf,
    total_blocks: usize,
    can_read: bool,
    can_write: bool,
    phantom: PhantomData<P>,
    weak_self: Weak<Inner<P>>,
}

impl<P: IoUringProvider> IoUringDisk<P> {
    fn do_read(&self, req: &Arc<BioReq>) -> Result<()> {
        if !self.0.can_read {
            return Err(errno!(EACCES, "read is not allowed"));
        }

        let (offset, _) = self.get_range_in_bytes(&req)?;

        let fd = Fd(self.0.fd as i32);
        let iovecs = req.access_mut_bufs_with(|bufs| {
            let iovecs: Vec<libc::iovec> = bufs
                .iter_mut()
                .map(|buf| {
                    let buf_slice = buf.as_slice_mut();
                    libc::iovec {
                        iov_base: buf_slice.as_mut_ptr() as _,
                        iov_len: buf_slice.len(),
                    }
                })
                .collect();

            // Note that it is necessary to wrap the Vec with Box. Otherwise,
            // the iovec_ptr will become invalid when the iovecs is moved into
            // the callback closure.
            Box::new(iovecs)
        });

        // TODO: fix this limitation
        const LINUX_IOVECS_MAX: usize = 1024;
        debug_assert!(iovecs.len() < LINUX_IOVECS_MAX);

        #[cfg(not(feature = "sgx"))]
        let (iovecs_ptr, iovecs_len) = { (iovecs.as_ptr() as _, iovecs.len()) };
        #[cfg(feature = "sgx")]
        let (iovecs_ptr, iovecs_len, untrusted_bufs, untrusted_iovecs) = {
            let mut untrusted_bufs = Vec::with_capacity(iovecs.len());
            let mut untrusted_iovecs = UntrustedBox::<[libc::iovec]>::new_slice(iovecs.as_slice());
            for (iov, mut untrusted_iov) in iovecs.iter().zip(untrusted_iovecs.iter_mut()) {
                let buf = UntrustedBox::<[u8]>::new_uninit_slice(iov.iov_len);
                untrusted_iov.iov_base = buf.as_ptr() as _;
                untrusted_bufs.push(buf);
            }
            (
                untrusted_iovecs.as_ptr() as _,
                untrusted_iovecs.len(),
                untrusted_bufs,
                untrusted_iovecs,
            )
        };

        #[cfg(feature = "sgx")]
        let mut recv_bufs: Vec<_> = iovecs
            .iter()
            .map(|iov| unsafe {
                std::slice::from_raw_parts_mut(iov.iov_base as *mut u8, iov.iov_len)
            })
            .collect();

        let complete_fn = {
            let self_ = self.clone_self();
            let req = req.clone();

            // Safety. All pointers contained in iovecs are still valid as the
            // buffers of the BIO request is valid.
            let send_iovecs = unsafe { MarkSend::new(iovecs) };
            #[cfg(feature = "sgx")]
            let send_u_iovecs = unsafe { MarkSend::new(untrusted_iovecs) };
            move |retval: i32| {
                // When the callback is invoked, the iovecs must have been
                // useless. And we call drop it safely.
                drop(send_iovecs);
                #[cfg(feature = "sgx")]
                drop(send_u_iovecs);
                // When the callback is invoked, IoUringDisk and its associated
                // resources are no longer needed.
                drop(self_);

                let resp = if retval >= 0 {
                    let expected_len = req.num_blocks() * BLOCK_SIZE;
                    // We don't expect short reads on regular files
                    debug_assert!(retval as usize == expected_len);
                    // Copy the buffer from untrusted
                    #[cfg(feature = "sgx")]
                    for (buf, u_buf) in recv_bufs.iter_mut().zip(untrusted_bufs.iter()) {
                        buf.copy_from_slice(u_buf);
                    }
                    Ok(())
                } else {
                    Err(Errno::from((-retval) as u32))
                };

                unsafe {
                    req.complete(resp);
                }
            }
        };
        let io_uring = P::io_uring();
        let io_handle = unsafe {
            io_uring.readv(
                fd,
                iovecs_ptr,
                iovecs_len as u32,
                offset as i64,
                0,
                complete_fn,
            )
        };
        // We don't need to keep the handle
        IoHandle::release(io_handle);
        Ok(())
    }

    fn do_write(&self, req: &Arc<BioReq>) -> Result<()> {
        if !self.0.can_write {
            return Err(errno!(EACCES, "write is not allowed"));
        }

        let (offset, _) = self.get_range_in_bytes(&req)?;

        let fd = Fd(self.0.fd as i32);
        let iovecs = req.access_bufs_with(|bufs| {
            let iovecs: Vec<libc::iovec> = bufs
                .iter()
                .map(|buf| {
                    let buf_slice = buf.as_slice();
                    libc::iovec {
                        iov_base: buf_slice.as_ptr() as *mut u8 as _,
                        iov_len: buf_slice.len(),
                    }
                })
                .collect();

            // Note that it is necessary to wrap the Vec with Box. Otherwise,
            // the iovec_ptr will become invalid when the iovecs is moved into
            // the callback closure.
            Box::new(iovecs)
        });

        // TODO: fix this limitation
        const LINUX_IOVECS_MAX: usize = 1024;
        debug_assert!(iovecs.len() < LINUX_IOVECS_MAX);

        #[cfg(not(feature = "sgx"))]
        let (iovecs_ptr, iovecs_len) = { (iovecs.as_ptr() as _, iovecs.len()) };
        #[cfg(feature = "sgx")]
        let (iovecs_ptr, iovecs_len, untrusted_bufs, untrusted_iovecs) = {
            let mut untrusted_bufs = Vec::with_capacity(iovecs.len());
            let mut untrusted_iovecs = UntrustedBox::<[libc::iovec]>::new_slice(iovecs.as_slice());
            for (iov, mut untrusted_iov) in iovecs.iter().zip(untrusted_iovecs.iter_mut()) {
                let write_buf =
                    unsafe { std::slice::from_raw_parts(iov.iov_base as *const u8, iov.iov_len) };
                let buf = UntrustedBox::<[u8]>::new_slice(write_buf);
                untrusted_iov.iov_base = buf.as_ptr() as _;
                untrusted_bufs.push(buf);
            }
            (
                untrusted_iovecs.as_ptr() as _,
                untrusted_iovecs.len(),
                untrusted_bufs,
                untrusted_iovecs,
            )
        };

        let complete_fn = {
            let self_ = self.clone_self();
            let req = req.clone();
            // Safety. All pointers contained in iovecs are still valid as the
            // buffers of the BIO request is valid.
            let send_iovecs = unsafe { MarkSend::new(iovecs) };
            #[cfg(feature = "sgx")]
            let send_u_iovecs = unsafe { MarkSend::new(untrusted_iovecs) };
            move |retval: i32| {
                // When the callback is invoked, the iovecs must have been
                // useless. And we call drop it safely.
                drop(send_iovecs);
                #[cfg(feature = "sgx")]
                drop(send_u_iovecs);
                #[cfg(feature = "sgx")]
                drop(untrusted_bufs);
                // When the callback is invoked, IoUringDisk and its associated
                // resources are no longer needed.
                drop(self_);

                let resp = if retval >= 0 {
                    let expected_len = req.num_blocks() * BLOCK_SIZE;
                    // We don't expect short writes on regular files
                    debug_assert!(retval as usize == expected_len);
                    Ok(())
                } else {
                    Err(Errno::from((-retval) as u32))
                };

                unsafe {
                    req.complete(resp);
                }
            }
        };
        let io_uring = P::io_uring();
        let io_handle = unsafe {
            io_uring.writev(
                fd,
                iovecs_ptr,
                iovecs_len as u32,
                offset as i64,
                0,
                complete_fn,
            )
        };
        // We don't need to keep the handle
        IoHandle::release(io_handle);

        Ok(())
    }

    fn do_flush(&self, req: &Arc<BioReq>) -> Result<()> {
        if !self.0.can_write {
            return Err(errno!(EACCES, "flush is not allowed"));
        }

        let fd = Fd(self.0.fd as i32);
        let is_datasync = true;
        let complete_fn = {
            let self_ = self.clone_self();
            let req = req.clone();
            move |retval: i32| {
                // When the callback is invoked, IoUringDisk and its associated
                // resources are no longer needed.
                drop(self_);

                let resp = if retval == 0 {
                    Ok(())
                } else if retval < 0 {
                    Err(Errno::from((-retval) as u32))
                } else {
                    panic!("impossible retval");
                };

                unsafe {
                    req.complete(resp);
                }
            }
        };
        let io_uring = P::io_uring();
        let io_handle = unsafe { io_uring.fsync(fd, is_datasync, complete_fn) };
        // We don't need to keep the handle
        IoHandle::release(io_handle);

        Ok(())
    }

    fn get_range_in_bytes(&self, req: &Arc<BioReq>) -> Result<(usize, usize)> {
        let begin_block = req.addr();
        let end_block = begin_block + req.num_blocks();
        if end_block > self.0.total_blocks {
            return Err(errno!(EINVAL, "invalid block range"));
        }
        let begin_offset = begin_block * BLOCK_SIZE;
        let end_offset = end_block * BLOCK_SIZE;
        Ok((begin_offset, end_offset))
    }

    // Returns a `Self` from `&self`.
    //
    // We pass the ownership of `Self` to the callback closure of an async
    // io_uring request to ensure that IoUringDisk and all its associated
    // resources (e.g., file descriptor) is valid while the async I/O request
    // is being processed.
    fn clone_self(&self) -> Self {
        Self(self.0.weak_self.upgrade().unwrap())
    }
}

impl<P: IoUringProvider> BlockDevice for IoUringDisk<P> {
    fn total_blocks(&self) -> usize {
        self.0.total_blocks
    }

    fn submit(&self, req: Arc<BioReq>) -> BioSubmission {
        // Update the status of req to submittted
        let submission = BioSubmission::new(req);

        // Try to initiate the I/O
        let req = submission.req();
        let type_ = req.type_();
        let res = match type_ {
            BioType::Read => self.do_read(req),
            BioType::Write => self.do_write(req),
            BioType::Flush => self.do_flush(req),
        };

        // If any error returns, then the request must have failed to submit. So
        // we set its status of "completed" here and set the response to the error.
        if let Err(e) = res {
            unsafe {
                req.complete(Err(e.errno()));
            }
        }

        submission
    }
}

impl<P: IoUringProvider> HostDisk for IoUringDisk<P> {
    fn from_options_and_file(options: &OpenOptions<Self>, file: File, path: &Path) -> Result<Self> {
        let fd = file.as_raw_fd();
        let total_blocks = options.total_blocks.unwrap_or_else(|| {
            let file_len = file.metadata().unwrap().len() as usize;
            assert!(file_len >= BLOCK_SIZE);
            file_len / BLOCK_SIZE
        });
        let can_read = options.read;
        let can_write = options.write;
        let path = path.to_owned();
        let inner = Inner {
            fd,
            file: Mutex::new(file),
            path,
            total_blocks,
            can_read,
            can_write,
            phantom: PhantomData,
            weak_self: Weak::new(),
        };
        let arc_inner = new_self_ref_arc!(inner);
        let new_self = Self(arc_inner);
        Ok(new_self)
    }

    fn path(&self) -> &Path {
        self.0.path.as_path()
    }
}

impl<P: IoUringProvider> Drop for IoUringDisk<P> {
    fn drop(&mut self) {
        // Ensure all data are peristed before the disk is dropped
        let mut file = self.0.file.lock().unwrap();
        let _ = file.flush();
    }
}

/// Mark an instance of type `T` as `Send`.
///
/// This is useful when an instance of type `T` is safe to send across threads,
/// but the marker trait Send cannot be implemented for T due to the
/// orphan rules.
pub struct MarkSend<T>(T);

impl<T> MarkSend<T> {
    /// Wrap an instance of type `T` so that it becomes `Send`.
    ///
    /// # Safety
    ///
    /// The user must make sure that it is indeed to send such a value across
    /// threads.
    pub unsafe fn new(inner: T) -> Self {
        Self(inner)
    }
}

unsafe impl<T> Send for MarkSend<T> {}

#[cfg(test)]
mod test {
    use super::*;
    use runtime::IoUringSingleton;

    fn test_setup() -> IoUringDisk<IoUringSingleton> {
        // As unit tests may run concurrently, they must operate on different
        // files. This helper function generates unique file paths.
        fn gen_unique_path() -> String {
            use std::sync::atomic::{AtomicU32, Ordering::Relaxed};

            static UT_ID: AtomicU32 = AtomicU32::new(0);

            let ut_id = UT_ID.fetch_add(1, Relaxed);
            format!("io_uring_disk{}.image", ut_id)
        }

        let total_blocks = 16;
        let path = gen_unique_path();
        IoUringDisk::<IoUringSingleton>::create(&path, total_blocks).unwrap()
    }

    fn test_teardown(disk: IoUringDisk<IoUringSingleton>) {
        let _ = std::fs::remove_file(disk.path());
    }

    block_device::gen_unit_tests!(test_setup, test_teardown);

    mod runtime {
        use super::*;
        use io_uring_callback::{Builder, IoUring};
        use lazy_static::lazy_static;

        pub struct IoUringSingleton;

        impl IoUringProvider for IoUringSingleton {
            fn io_uring() -> &'static IoUring {
                &*IO_URING
            }
        }

        lazy_static! {
            static ref IO_URING: Arc<IoUring> = {
                let ring = Arc::new(Builder::new().build(256).unwrap());
                unsafe {
                    ring.start_enter_syscall_thread();
                }
                std::thread::spawn({
                    let ring = ring.clone();
                    move || loop {
                        let min_complete = 1;
                        let polling_retries = 10000;
                        ring.poll_completions(min_complete, polling_retries);
                    }
                });
                ring
            };
        }
    }
}
