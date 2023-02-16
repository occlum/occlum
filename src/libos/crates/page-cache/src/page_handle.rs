use crate::prelude::*;
use crate::Page;

use std::fmt::{Debug, Formatter};
use std::ptr::NonNull;
use std::sync::Arc;

/// Page handle.
#[derive(Clone)]
pub struct PageHandle<K: PageKey, A: PageAlloc>(pub(crate) Arc<Inner<K, A>>);

pub(crate) struct Inner<K: PageKey, A: PageAlloc> {
    key: K,
    pollee: Pollee,
    state_and_page: Mutex<(PageState, Page<A>)>,
}

impl<K: PageKey, A: PageAlloc> PageHandle<K, A> {
    /// Create a new page handle with a new allocated page.
    pub(crate) fn new(key: K) -> Option<Self> {
        Page::new().map(|new_page| {
            Self(Arc::new(Inner {
                key,
                pollee: Pollee::new(Events::empty()),
                state_and_page: Mutex::new((PageState::Uninit, new_page)),
            }))
        })
    }

    /// Return the page ID.
    #[inline]
    pub fn key(&self) -> K {
        self.0.key
    }

    /// Return the pollee.
    pub fn pollee(&self) -> &Pollee {
        &self.0.pollee
    }

    /// Return the lock guard of `state_and_page`.
    pub fn lock(&'a self) -> PageHandleGuard<'a, A> {
        PageHandleGuard(self.0.state_and_page.lock())
    }
}

impl<K: PageKey, A: PageAlloc> Debug for PageHandle<K, A> {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        let page_guard = self.lock();
        write!(
            f,
            "PageHandle {{ key: {:?}, state: {:?} }}",
            self.key(),
            page_guard.state()
        )
    }
}

/// The lock guard for the page handle.
pub struct PageHandleGuard<'a, A: PageAlloc>(MutexGuard<'a, (PageState, Page<A>)>);

impl<'a, A: PageAlloc> PageHandleGuard<'a, A> {
    /// Return current state of page.
    #[inline]
    pub fn state(&self) -> PageState {
        self.0 .0
    }

    /// Set a new state to current page.
    ///
    /// Ensure legal state transition before set new state for a page.
    pub fn set_state(&mut self, new_state: PageState) {
        fn allow_state_transition(curr_state: PageState, new_state: PageState) -> bool {
            match (curr_state, new_state) {
                (_, PageState::Uninit) => false,
                (PageState::Uninit | PageState::Dirty, PageState::UpToDate) => false,
                (PageState::Fetching | PageState::Flushing, PageState::Dirty) => false,
                (state, PageState::Fetching) if state != PageState::Uninit => false,
                (state, PageState::Flushing) if state != PageState::Dirty => false,
                _ => true,
            }
        }
        debug_assert!(allow_state_transition(self.state(), new_state));

        self.0 .0 = new_state;
    }

    /// Return a pointer to the underlying page buffer.
    #[inline]
    pub fn as_ptr(&self) -> NonNull<u8> {
        self.0 .1.as_ptr()
    }

    /// Return a slice.
    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        self.0 .1.as_slice()
    }

    /// Return a mutable slice.
    #[inline]
    pub fn as_slice_mut(&mut self) -> &mut [u8] {
        self.0 .1.as_slice_mut()
    }
}
