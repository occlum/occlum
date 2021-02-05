use std::marker::PhantomData;
#[cfg(feature = "sgx")]
use std::prelude::v1::*;
#[cfg(not(feature = "sgx"))]
use std::sync::{Arc, Mutex, RwLock};
#[cfg(feature = "sgx")]
use std::sync::{Arc, SgxMutex as Mutex, SgxRwLock as RwLock};

use crate::event::waiter::{Waiter, WaiterQueue};
use crate::file::tracker::SeqRdTracker;
use crate::page_cache::{AsFd, Page, PageCache, PageHandle, PageState};
use crate::util::{align_down, align_up};

pub use self::flusher::Flusher;

use io_uring_callback::{Fd, Handle, IoUring};
#[cfg(feature = "sgx")]
use sgx_untrusted_alloc::UntrustedAllocator;

mod flusher;
mod tracker;

/// An instance of file with async APIs.
pub struct AsyncFile<Rt: AsyncFileRt + ?Sized> {
    fd: i32,
    len: RwLock<usize>,
    can_read: bool,
    can_write: bool,
    seq_rd_tracker: SeqRdTracker,
    waiter_queue: WaiterQueue,
    phantom_data: PhantomData<Rt>,
}

/// The runtime support for AsyncFile.
///
/// AsyncFile cannot work on its own: it leverages PageCache to accelerate I/O,
/// needs Flusher to persist data, and eventually depends on IoUring to perform
/// async I/O. This trait provides a common interface for user-implemented runtimes
/// that support AsyncFile.
pub trait AsyncFileRt: Send + Sync + 'static {
    /// Returns the io_uring instance.
    fn io_uring() -> &'static IoUring;
    fn page_cache() -> &'static PageCache;
    fn flusher() -> &'static Flusher<Self>;
    fn auto_flush();
}

impl<Rt: AsyncFileRt + ?Sized> AsyncFile<Rt> {
    /// Open a file at a given path.
    ///
    /// The three arguments have the same meaning as the open syscall.
    pub fn open(mut path: String, flags: i32, mode: u32) -> Result<Arc<Self>, i32> {
        let (can_read, can_write) = if flags & libc::O_WRONLY != 0 {
            (false, true)
        } else if flags & libc::O_RDWR != 0 {
            (true, true)
        } else {
            // libc::O_RDONLY = 0
            (true, false)
        };

        let fd = unsafe {
            let c_path = std::ffi::CString::new(path).unwrap();
            let c_path_ptr = c_path.as_bytes_with_nul().as_ptr() as _;
            let flags = if flags & libc::O_WRONLY != 0 {
                (flags & !libc::O_WRONLY) | libc::O_RDWR
            } else {
                flags
            };
            #[cfg(not(feature = "sgx"))]
            let fd = libc::open(c_path_ptr, flags, mode);
            #[cfg(feature = "sgx")]
            let fd = libc::ocall::open64(c_path_ptr, flags, mode as i32);
            fd
        };
        if fd < 0 {
            return Err(errno());
        }

        #[cfg(not(feature = "sgx"))]
        let len = unsafe { libc::lseek(fd, 0, libc::SEEK_END) };
        #[cfg(feature = "sgx")]
        let len = unsafe { libc::ocall::lseek(fd, 0, libc::SEEK_END) };
        if len < 0 {
            return Err(errno());
        }

        Ok(Arc::new(Self {
            fd,
            len: RwLock::new(len as usize),
            can_read,
            can_write,
            seq_rd_tracker: SeqRdTracker::new(),
            waiter_queue: WaiterQueue::new(),
            phantom_data: PhantomData,
        }))
    }

    pub async fn read_at(self: &Arc<Self>, offset: usize, buf: &mut [u8]) -> i32 {
        if !self.can_read {
            return -libc::EBADF;
        }
        if buf.len() == 0 {
            return 0;
        }

        // Prevent offset calculation from overflow
        if offset >= usize::max_value() / 2 {
            return -libc::EINVAL;
        }
        // Prevent the return length (i32) from overflow
        if buf.len() > i32::max_value() as usize {
            return -libc::EINVAL;
        }

        // Fast path
        let retval = self.try_read_at(offset, buf);
        if retval != -libc::EAGAIN {
            return retval;
        }

        // Slow path
        let waiter = Waiter::new();
        self.waiter_queue.enqueue(&waiter);
        let retval = loop {
            let retval = self.try_read_at(offset, buf);
            if retval != -libc::EAGAIN {
                break retval;
            }

            waiter.wait().await;
        };
        self.waiter_queue.dequeue(&waiter);
        retval
    }

    fn try_read_at(self: &Arc<Self>, offset: usize, buf: &mut [u8]) -> i32 {
        let file_len = *self.len.read().unwrap();

        // For reads beyond the end of the file
        if offset >= file_len {
            // EOF
            return 0;
        }

        // For reads within the bound of the file
        let file_remaining = file_len - offset;
        let buf_len = buf.len().min(file_remaining);
        let buf = &mut buf[..buf_len];

        // Determine if it is a sequential read and how much data to prefetch
        let seq_rd = self.seq_rd_tracker.accept(offset, buf.len());
        let prefetch_len = {
            let prefetch_len = seq_rd.as_ref().map_or(0, |seq_rd| seq_rd.prefetch_size());
            let max_prefetch_len = file_remaining - buf.len();
            prefetch_len.min(max_prefetch_len)
        };

        // Fetch the data to the page cache and copy the data of the first ready pages
        // in the page cache to the output buffer.
        let mut read_nbytes = 0;
        self.fetch_pages(offset, buf_len, prefetch_len, |page_handle: &PageHandle| {
            let page_slice = unsafe { page_handle.page().as_slice() };
            let inner_offset = offset + read_nbytes - page_handle.offset();
            let page_remain = Page::size() - inner_offset;

            let buf_remain = buf_len - read_nbytes;
            let copy_size = buf_remain.min(page_remain);
            let src_buf = &page_slice[inner_offset..inner_offset + copy_size];
            let target_buf = &mut buf[read_nbytes..read_nbytes + copy_size];
            target_buf.copy_from_slice(src_buf);

            read_nbytes += copy_size;
        });

        if read_nbytes > 0 {
            seq_rd.map(|seq_rd| seq_rd.complete(read_nbytes));
            read_nbytes as i32
        } else {
            -libc::EAGAIN
        }
    }

    // Fetch and prefetch pages.
    //
    // The first pages in the fetch range [offset, offset + len) that are ready to read are passed
    // to a closure so that the caller can access the data in these pages. Note that the state of the
    // page is locked while the closure is being executed.
    //
    // The pages that are within the range [offset, offset + len + prefetch_len] will be fetched into
    // the page cache, if they are not present in the page cache.
    //
    // The procedure works in two phases. The first phase is fetching, in which we iterate
    // the first pages that are ready to read. These pages are passed to the access closure
    // one-by-one. Upon reaching the first page that cannot be read or beyond the fetching
    // range [offset, offset + len), we transit to the second phase: prefetching. In this
    // phase, we will try out our best to bring the pages into the page cache,
    // issueing async reads if needed.
    fn fetch_pages(
        self: &Arc<Self>,
        offset: usize,
        len: usize,
        prefetch_len: usize,
        mut access_fn: impl FnMut(&PageHandle),
    ) {
        // If the first stage, the value is true; if the second stage, false.
        let mut should_call_access_fn = true;
        // Prepare for async read that fetches multiple consecutive pages
        let mut consecutive_pages = Vec::new();

        // Enter the loop that fetches and prefetches pages.
        let page_cache = Rt::page_cache();
        let page_begin = align_down(offset, Page::size());
        let page_end = align_up(offset + len + prefetch_len, Page::size());
        let fetch_end = align_up(offset + len, Page::size());
        for page_offset in (page_begin..page_end).step_by(Page::size()) {
            if should_call_access_fn && page_offset >= fetch_end {
                should_call_access_fn = false;
            }

            let page = page_cache.acquire(self, page_offset).unwrap();
            let mut state = page.state();
            if should_call_access_fn {
                // The fetching phase
                match *state {
                    PageState::UpToDate | PageState::Dirty | PageState::Flushing => {
                        // Invoke the access function
                        (access_fn)(&page);

                        drop(state);
                        page_cache.release(page);
                    }
                    PageState::Uninit => {
                        // Start prefetching
                        *state = PageState::Fetching;
                        drop(state);
                        consecutive_pages.push(page);

                        // Transit to the prefetching phase
                        should_call_access_fn = false;
                    }
                    PageState::Fetching => {
                        // We do nothing here
                        drop(state);
                        page_cache.release(page);

                        // Transit to the prefetching phase
                        should_call_access_fn = false;
                    }
                }
            } else {
                // The prefetching phase
                match *state {
                    PageState::Uninit => {
                        // Add one more page to prefetch
                        *state = PageState::Fetching;
                        drop(state);
                        consecutive_pages.push(page);
                    }
                    PageState::UpToDate
                    | PageState::Dirty
                    | PageState::Flushing
                    | PageState::Fetching => {
                        drop(state);
                        page_cache.release(page);

                        // When reaching the end of consecutive pages, start the I/O
                        if consecutive_pages.len() > 0 {
                            self.fetch_consecutive_pages(consecutive_pages);
                            consecutive_pages = Vec::new();
                        }
                    }
                }
            }
        }
        // When reaching the end of consecutive pages, start the I/O
        if consecutive_pages.len() > 0 {
            self.fetch_consecutive_pages(consecutive_pages);
        }
    }

    fn fetch_consecutive_pages(self: &Arc<Self>, consecutive_pages: Vec<PageHandle>) {
        debug_assert!(!consecutive_pages.is_empty());
        debug_assert!(consecutive_pages.windows(2).all(|two_pages| {
            let (p0, p1) = (&two_pages[0], &two_pages[1]);
            p0.offset() + Page::size() == p1.offset()
        }));
        debug_assert!(consecutive_pages
            .iter()
            .all(|page| { *page.state() == PageState::Fetching }));

        let first_offset = consecutive_pages[0].offset();
        let self_ = self.clone();
        let iovecs = Box::new(
            consecutive_pages
                .iter()
                .map(|page_handle| libc::iovec {
                    iov_base: page_handle.page().as_mut_ptr() as _,
                    iov_len: Page::size(),
                })
                .collect::<Vec<libc::iovec>>(),
        );
        #[cfg(not(feature = "sgx"))]
        let (iovecs_ptr, iovecs_len) = ((*iovecs).as_ptr(), (*iovecs).len());
        #[cfg(feature = "sgx")]
        let (iovecs_ptr, iovecs_len, allocator, iovecs_ptr_u64, t_iovecs_ptr_u64) = {
            let iovecs_len = (*iovecs).len();
            let t_iovecs_ptr = (*iovecs).as_ptr();
            let iovecs_size = iovecs_len * core::mem::size_of::<libc::iovec>();
            let size = iovecs_size + iovecs_len * Page::size();
            let allocator = UntrustedAllocator::new(size, 8).unwrap();
            let iovecs_ptr = allocator.as_mut_ptr() as *mut libc::iovec;
            let data_ptr = unsafe { iovecs_ptr.add(iovecs_size) as *mut u8 };
            for idx in 0..iovecs_len {
                unsafe {
                    *iovecs_ptr.add(idx) = libc::iovec {
                        iov_base: data_ptr.add(idx * Page::size()) as _,
                        iov_len: Page::size(),
                    };
                }
            }
            (
                iovecs_ptr,
                iovecs_len,
                allocator,
                iovecs_ptr as u64,
                t_iovecs_ptr as u64,
            )
        };

        struct IovecsBox(Box<Vec<libc::iovec>>);
        unsafe impl Send for IovecsBox {}
        let iovecs_box = IovecsBox(iovecs);

        let handle_store: Arc<Mutex<Option<Handle>>> = Arc::new(Mutex::new(None));
        let handle_store2 = handle_store.clone();

        let callback = move |retval| {
            let page_cache = Rt::page_cache();
            let read_nbytes = if retval >= 0 { retval } else { 0 } as usize;
            for page in consecutive_pages {
                let page_offset = page.offset();
                debug_assert!(page_offset >= first_offset);

                // For a partial read, fill zeros or in the remaining part of the page.
                // TODO: are there partial reads that should not fill zeros?
                let page_valid_nbytes = if first_offset + read_nbytes > page_offset {
                    (first_offset + read_nbytes - page_offset).min(Page::size())
                } else {
                    0
                };
                if page_valid_nbytes < Page::size() {
                    let page_slice = unsafe { page.page().as_slice_mut() };
                    page_slice[page_valid_nbytes..].fill(0);
                }

                // Update page state
                {
                    let mut state = page.state();
                    debug_assert!(*state == PageState::Fetching);
                    *state = PageState::UpToDate;
                }
                page_cache.release(page);
            }
            self_.waiter_queue.wake_all();

            #[cfg(feature = "sgx")]
            {
                let iovecs_ptr = iovecs_ptr_u64 as *const libc::iovec;
                let t_iovecs_ptr = t_iovecs_ptr_u64 as *mut libc::iovec;
                for idx in 0..iovecs_len {
                    unsafe {
                        assert!((*t_iovecs_ptr.add(idx)).iov_len == Page::size());
                        std::ptr::copy_nonoverlapping(
                            (*iovecs_ptr.add(idx)).iov_base,
                            (*t_iovecs_ptr.add(idx)).iov_base,
                            (*t_iovecs_ptr.add(idx)).iov_len,
                        );
                    }
                }
                drop(allocator);
            }
            drop(iovecs_box);
            drop(handle_store);
        };
        let io_uring = Rt::io_uring();
        let handle = unsafe {
            io_uring.readv(
                Fd(self.fd),
                iovecs_ptr,
                iovecs_len as u32,
                first_offset as i64,
                0,
                callback,
            )
        };
        let mut guard = handle_store2.lock().unwrap();
        guard.replace(handle);
    }

    pub async fn write_at(self: &Arc<Self>, offset: usize, buf: &[u8]) -> i32 {
        if !self.can_write {
            return -libc::EBADF;
        }
        if buf.len() == 0 {
            return 0;
        }

        // Fast path
        let retval = self.try_write(offset, buf);
        if retval != -libc::EAGAIN {
            return retval;
        }

        // Slow path
        let waiter = Waiter::new();
        self.waiter_queue.enqueue(&waiter);
        let retval = loop {
            let retval = self.try_write(offset, buf);
            if retval != -libc::EAGAIN {
                break retval;
            }

            waiter.wait().await;
        };
        self.waiter_queue.dequeue(&waiter);
        retval
    }

    fn try_write(self: &Arc<Self>, offset: usize, buf: &[u8]) -> i32 {
        // Prevent offset calculation from overflow
        if offset >= usize::max_value() / 2 {
            return -libc::EINVAL;
        }
        // Prevent the return length (i32) from overflow
        if buf.len() > i32::max_value() as usize {
            return -libc::EINVAL;
        }

        let mut new_dirty_pages = false;
        let mut write_nbytes = 0;
        let page_cache = Rt::page_cache();
        let page_begin = align_down(offset, Page::size());
        let page_end = align_up(offset + buf.len(), Page::size());
        for page_offset in (page_begin..page_end).step_by(Page::size()) {
            let page_handle = page_cache.acquire(self, page_offset).unwrap();
            let inner_offset = offset + write_nbytes - page_offset;

            let copy_size = {
                let page_remain = Page::size() - inner_offset;
                let buf_remain = buf.len() - write_nbytes;
                buf_remain.min(page_remain)
            };
            let to_write_full_page = copy_size == Page::size();

            let mut do_write = || {
                let page_slice = unsafe { page_handle.page().as_slice_mut() };

                let src_buf = &buf[write_nbytes..write_nbytes + copy_size];
                let dst_buf = &mut page_slice[inner_offset..inner_offset + copy_size];
                dst_buf.copy_from_slice(src_buf);

                write_nbytes += copy_size;
            };

            let mut state = page_handle.state();
            match *state {
                PageState::UpToDate => {
                    (do_write)();

                    *state = PageState::Dirty;
                    drop(state);
                    page_cache.release(page_handle);

                    new_dirty_pages = true;
                }
                PageState::Dirty => {
                    (do_write)();

                    drop(state);
                    page_cache.release(page_handle);
                }
                PageState::Uninit if to_write_full_page => {
                    (do_write)();

                    *state = PageState::Dirty;
                    drop(state);
                    page_cache.release(page_handle);

                    new_dirty_pages = true;
                }
                PageState::Uninit => {
                    *state = PageState::Fetching;
                    drop(state);

                    self.fetch_consecutive_pages(vec![page_handle]);
                    break;
                }
                PageState::Fetching | PageState::Flushing => {
                    // We do nothing here
                    drop(state);
                    page_cache.release(page_handle);

                    break;
                }
            }
        }

        if new_dirty_pages {
            Rt::auto_flush();
        }

        if write_nbytes > 0 {
            // Update file length if necessary
            let mut file_len = self.len.write().unwrap();
            if offset + write_nbytes > *file_len {
                *file_len = offset + write_nbytes;
            }

            write_nbytes as i32
        } else {
            -libc::EAGAIN
        }
    }

    pub async fn flush(&self) {
        loop {
            const FLUSH_BATCH_SIZE: usize = 64;
            let num_flushed = Rt::flusher().flush_by_fd(self.fd, FLUSH_BATCH_SIZE).await;
            if num_flushed == 0 {
                return;
            }
        }
    }

    pub(crate) fn waiter_queue(&self) -> &WaiterQueue {
        &self.waiter_queue
    }
}

impl<Rt: AsyncFileRt + ?Sized> AsFd for AsyncFile<Rt> {
    fn as_fd(&self) -> i32 {
        self.fd
    }
}

impl<Rt: AsyncFileRt + ?Sized> Drop for AsyncFile<Rt> {
    fn drop(&mut self) {
        unsafe {
            #[cfg(not(feature = "sgx"))]
            libc::close(self.fd);
            #[cfg(feature = "sgx")]
            libc::ocall::close(self.fd);
        }
    }
}

#[cfg(not(feature = "sgx"))]
fn errno() -> i32 {
    unsafe {
        *(libc::__errno_location())
        // *(libc::__error())
    }
}

#[cfg(feature = "sgx")]
fn errno() -> i32 {
    libc::errno()
}
