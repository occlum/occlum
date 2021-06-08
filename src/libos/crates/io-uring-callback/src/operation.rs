//! Types that describe Async I/O operations.
#[cfg(feature = "sgx")]
use std::prelude::v1::*;
#[cfg(not(feature = "sgx"))]
use std::sync::Mutex;
#[cfg(feature = "sgx")]
use std::sync::SgxMutex as Mutex;
use std::task::Waker;

use atomic::{Atomic, Ordering};

pub type Callback = Box<dyn FnOnce(i32) + Send + 'static>;

pub struct Token {
    state: Atomic<State>,
    callback: Mutex<Option<Callback>>,
    waker: Mutex<Option<Waker>>,
}

impl Token {
    pub fn new(callback: impl FnOnce(i32) + Send + 'static) -> Self {
        let state = Atomic::new(State::Submitted);
        let callback = Mutex::new(Some(Box::new(callback) as _));
        let waker = Mutex::new(None);
        Self {
            state,
            callback,
            waker,
        }
    }

    pub fn complete(&self, retval: i32) -> Box<dyn FnOnce(i32) + 'static> {
        loop {
            let old_state = self.state.load(Ordering::Acquire);
            debug_assert!(old_state == State::Submitted);
            let new_state = State::Completed(retval);
            if self
                .state
                .compare_exchange(old_state, new_state, Ordering::Release, Ordering::Relaxed)
                .is_ok()
            {
                self.wake();
                return self.take_callback();
            }
        }
    }

    #[allow(dead_code)]
    pub fn cancel(&self) {
        todo!();
    }

    #[allow(dead_code)]
    pub fn is_cancalling(&self) -> bool {
        self.state.load(Ordering::Acquire) == State::Cancelling
    }

    pub fn is_cancelled(&self) -> bool {
        self.state.load(Ordering::Acquire) == State::Cancelled
    }

    pub fn is_completed(&self) -> bool {
        self.retval().is_some()
    }

    pub fn retval(&self) -> Option<i32> {
        match self.state.load(Ordering::Acquire) {
            State::Completed(retval) => Some(retval),
            _ => None,
        }
    }

    pub fn set_waker(&self, waker: Waker) {
        let mut guard = self.waker.lock().unwrap();
        debug_assert!((*guard).is_none());
        (*guard).replace(waker);
    }

    fn wake(&self) {
        let mut guard = self.waker.lock().unwrap();
        if let Some(waker) = (*guard).take() {
            waker.wake()
        }
    }

    fn take_callback(&self) -> Box<dyn FnOnce(i32) + 'static> {
        let mut callback_opt = self.callback.lock().unwrap();
        callback_opt.take().unwrap()
    }
}

impl Drop for Token {
    fn drop(&mut self) {
        debug_assert!(self.is_completed() || self.is_cancelled());
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum State {
    Submitted,
    Completed(i32),
    Cancelling,
    Cancelled,
}
