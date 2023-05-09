//! This crate provides the abstractions for page cache.
//!
//! page-cache provides caching mechanism for block devices. Similar to Linux Buffer Cache,
//! the goal of page-cache is to minimize disk I/O by storing data (page/block granularity)
//! in physical memory that would otherwise require disk access.
//!
//! Typically, users can create a new `PageCache` and use `PageHandle` to manage pages.
//! Users can define their own page allocator and flusher by implementing `PageAlloc` and `PageCacheFlusher`.
//!
//! page-cache can both run on Linux or SGX environment.
//!
//! # Usage example
//!
//! ```
//! use page_cache::{PageAlloc, PageCache, PageCacheFlusher, impl_fixed_size_page_alloc};
//!
//! // Define own flusher
//! struct MyFlusher {/* omit struct */}
//! impl PageCacheFlusher for MyFlusher {/* omit impl */}
//!
//! // Define own page allocator
//! struct MyAllocator {/* omit struct */}
//! impl PageAlloc for MyAllocator {/* omit impl */}
//! // Or use macro to create a test-purpose fixed-size allocator
//! // impl_fixed_size_page_alloc!{MyAllocator, 1024}
//!
//! // Create a new page cache given a page key, an allocator and a flusher
//! let page_cache = PageCache<usize, MyAllocator>::new(MyFlusher);
//!
//! // Acquire a page handle with a page key
//! let page_handle = page_cache.acquire(0).unwrap();
//!
//! // Lock the page handle before manipulating the page
//! let page_guard = page_handle.lock();
//! let page_state = page_guard.state();
//! let page_slice = page_guard.as_slice();
//! /* Read or write the page content and alter the page state */
//! drop(page_guard);
//!
//! // Release the page handle
//! page_cache.release(page_handle);
//!
//! ```
#![cfg_attr(feature = "sgx", no_std)]
#![feature(async_closure)]
#![feature(const_fn_trait_bound)]
#![feature(get_mut_unchecked)]
#![feature(in_band_lifetimes)]
#![feature(map_first_last)]
#![feature(new_uninit)]
#![feature(slice_group_by)]

#[cfg(feature = "sgx")]
extern crate sgx_types;
#[cfg(feature = "sgx")]
#[macro_use]
extern crate sgx_tstd as std;
#[cfg(feature = "sgx")]
extern crate sgx_libc as libc;

#[macro_use]
extern crate log;

extern crate lru;

mod cached_disk;
mod page;
mod page_alloc;
mod page_cache;
mod page_evictor;
mod page_handle;
mod page_state;
mod prelude;

pub use self::cached_disk::CachedDisk;
use self::page::Page;
pub use self::page_alloc::{FixedSizePageAlloc, PageAlloc};
pub use self::page_cache::{PageCache, PageCacheFlusher, PageKey};
use self::page_evictor::PageEvictor;
pub use self::page_handle::{PageHandle, PageHandleGuard};
pub use self::page_state::PageState;
