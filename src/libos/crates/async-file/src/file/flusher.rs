use std::marker::PhantomData;
#[cfg(feature = "sgx")]
use std::prelude::v1::*;
use std::sync::Arc;

use async_io::poll::{Events, Pollee};
use futures::future::BoxFuture;
use io_uring_callback::Fd;
use itertools::Itertools;
#[cfg(feature = "sgx")]
use sgx_untrusted_alloc::UntrustedAllocator;

use crate::file::AsyncFileRt;
use crate::page_cache::{Page, PageHandle, PageState};

/// Flush dirty pages in a page cache.
pub struct Flusher<Rt: AsyncFileRt + ?Sized> {
    pollee: Arc<Pollee>,
    phantom_data: PhantomData<Rt>,
}

impl<Rt: AsyncFileRt + ?Sized> Flusher<Rt> {
    pub fn new() -> Self {
        Self {
            pollee: Arc::new(Pollee::new(Events::IN | Events::OUT)),
            phantom_data: PhantomData,
        }
    }

    pub async fn flush_by_fd(&self, fd: i32, max_pages: usize) -> usize {
        let dirty_pages = Rt::page_cache().evict_dirty_pages_by_fd(fd, max_pages);
        self.do_flush(dirty_pages).await
    }

    pub async fn flush(&self, max_pages: usize) -> usize {
        let dirty_pages = Rt::page_cache().evict_dirty_pages(max_pages);
        self.do_flush(dirty_pages).await
    }

    async fn do_flush(&self, mut dirty_pages: Vec<PageHandle>) -> usize {
        let page_cache = Rt::page_cache();
        // Remove all false positives in the supposed-to-be-dirty pages
        dirty_pages
            .drain_filter(|page| {
                let should_remove = {
                    let mut state = page.state();
                    if *state == PageState::Dirty {
                        *state = PageState::Flushing;
                        false
                    } else {
                        true
                    }
                };
                should_remove
            })
            .for_each(|non_dirty_page| {
                page_cache.release(non_dirty_page);
            });

        let num_dirty_pages = dirty_pages.len();
        if num_dirty_pages == 0 {
            return 0;
        }

        // Sort the pages so that we can easily merge small writes into larger ones
        dirty_pages.sort_by_key(|page| page.key());

        // Flush the dirty pages one fd at a time
        let mut futures: Vec<BoxFuture<'static, i32>> = Vec::new();
        dirty_pages
            .into_iter()
            .group_by(|page| page.fd())
            .into_iter()
            .for_each(|(fd, dirty_pages_of_a_fd)| {
                self.flush_dirty_pages_of_a_fd(fd, dirty_pages_of_a_fd, &mut futures);
            });
        for future in futures {
            future.await;
        }
        num_dirty_pages
    }

    fn flush_dirty_pages_of_a_fd(
        &self,
        fd: i32,
        mut iter: impl Iterator<Item = PageHandle>,
        futures: &mut Vec<BoxFuture<'static, i32>>,
    ) {
        let mut first_page_opt = iter.next();
        // Scan the dirty pages to group them into consecutive pages
        loop {
            // The first one in the consecutive pages
            let first_page = match first_page_opt {
                Some(first_page) => first_page,
                None => {
                    break;
                }
            };
            let first_offset = first_page.offset();

            // Collet the remaining consecutive pages
            let mut consecutive_pages = vec![first_page];
            let mut next_offset = first_offset + Page::size();
            loop {
                let next_page = match iter.next() {
                    Some(next_page) => next_page,
                    None => {
                        first_page_opt = None;
                        break;
                    }
                };
                if next_page.offset() != next_offset {
                    first_page_opt = Some(next_page);
                    break;
                }

                consecutive_pages.push(next_page);
                next_offset += Page::size();
            }

            let future =
                self.flush_consecutive_dirty_pages_of_a_fd(fd, first_offset, consecutive_pages);
            futures.push(future);
        }
    }

    fn flush_consecutive_dirty_pages_of_a_fd(
        &self,
        fd: i32,
        offset: usize,
        mut consecutive_pages: Vec<PageHandle>,
    ) -> BoxFuture<'static, i32> {
        // TODO: I don't think this Box is necessary (at least for non-SGX build)
        let iovecs: Box<Vec<libc::iovec>> = Box::new(
            consecutive_pages
                .iter()
                .map(|page| libc::iovec {
                    iov_base: page.page().as_mut_ptr() as _,
                    iov_len: Page::size(),
                })
                .collect(),
        );
        #[cfg(not(feature = "sgx"))]
        let (iovecs_ptr, iovecs_len) = ((*iovecs).as_ptr(), (*iovecs).len());
        #[cfg(feature = "sgx")]
        let (iovecs_ptr, iovecs_len, allocator) = {
            let iovecs_len = (*iovecs).len();
            let t_iovecs_ptr = (*iovecs).as_ptr();
            let iovecs_size = iovecs_len * core::mem::size_of::<libc::iovec>();
            let allocator = UntrustedAllocator::new(iovecs_size, 8).unwrap();
            let iovecs_ptr = allocator.as_mut_ptr() as *mut libc::iovec;
            unsafe {
                std::ptr::copy_nonoverlapping(t_iovecs_ptr, iovecs_ptr, iovecs_len);
            }
            (iovecs_ptr, iovecs_len, allocator)
        };

        struct IovecsBox(Box<Vec<libc::iovec>>);
        unsafe impl Send for IovecsBox {}
        let iovecs_box = IovecsBox(iovecs);

        let complete_fn = {
            let page_cache = Rt::page_cache();
            let flusher_pollee = self.pollee.clone();
            move |retval: i32| {
                // TODO: handle partial writes or error
                assert!(retval as usize == consecutive_pages.len() * Page::size());

                for page in consecutive_pages {
                    let mut state = page.state();
                    match *state {
                        PageState::Flushing => {
                            *state = PageState::UpToDate;
                        }
                        _ => unreachable!(),
                    };
                    drop(state);
                    page_cache.release(page);
                }
                flusher_pollee.add_events(Events::IN | Events::OUT);

                #[cfg(feature = "sgx")]
                drop(allocator);
                drop(iovecs_box);
            }
        };
        let io_uring = Rt::io_uring();
        let handle = unsafe {
            io_uring.writev(
                Fd(fd),
                iovecs_ptr,
                iovecs_len as u32,
                offset as i64,
                0,
                complete_fn,
            )
        };
        // TODO: I don't think this Box is necessary
        Box::pin(handle)
    }

    pub(crate) fn pollee(&self) -> &Pollee {
        &self.pollee
    }
}
