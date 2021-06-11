use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll, Waker};
cfg_if::cfg_if! {
    if #[cfg(feature = "sgx")] {
        use std::prelude::v1::*;
        use std::sync::SgxMutex as Mutex;
    } else {
        use std::sync::Mutex;
    }
}

/// The handle to an I/O request pushed to the submission queue of an io_uring instance.
pub struct IoHandle(Arc<IoToken>);

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum IoState {
    Submitted,
    Completed(i32),
    Cancelling,
    Cancelled,
}

impl IoHandle {
    pub(crate) fn new(token: Arc<IoToken>) -> Self {
        Self(token)
    }

    /// Returns the state of the I/O request.
    pub fn state(&self) -> IoState {
        self.0.state()
    }

    /// Returns the return value of the I/O request if it is completed.
    pub fn retval(&self) -> Option<i32> {
        self.0.retval()
    }

    /// Cancel the I/O request.
    ///
    /// This is NOT implemented, yet.
    pub fn cancel(&self) {
        self.0.cancel()
    }
}

impl Unpin for IoHandle {}

impl Future for IoHandle {
    type Output = i32;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut inner = self.0.inner.lock().unwrap();
        match inner.state {
            IoState::Completed(retval) => Poll::Ready(retval),
            IoState::Submitted => {
                inner.waker = Some(cx.waker().clone());
                Poll::Pending
            }
            _ => {
                todo!("handle cancel-related states");
            }
        }
    }
}

impl Drop for IoHandle {
    fn drop(&mut self) {
        // The user cannot drop a handle without completing or canceling it first.
        assert!(matches!(self.state(), IoState::Completed(_) | IoState::Cancelled));
    }
}

/// A token representing an on-going I/O request.
///
/// Tokens and handles are basically the same thing---an on-going I/O request. The main difference
/// is that handles are used by users, while tokens are used internally.
pub(crate) struct IoToken {
    inner: Mutex<Inner>,
}

impl IoToken {
    pub fn new(callback: impl FnOnce(i32) + Send + 'static) -> Self {
        let inner = Mutex::new(Inner::new(callback));
        Self { inner }
    }

    pub fn state(&self) -> IoState {
        let inner = self.inner.lock().unwrap();
        inner.state()
    }

    pub fn retval(&self) -> Option<i32> {
        let inner = self.inner.lock().unwrap();
        inner.retval()
    }

    pub fn complete(&self, retval: i32) {
        let mut inner = self.inner.lock().unwrap();
        inner.complete(retval)
    }

    pub fn cancel(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.cancel()
    }
}

struct Inner {
    state: IoState,
    callback: Option<Callback>,
    waker: Option<Waker>,
}

type Callback = Box<dyn FnOnce(i32) + Send + 'static>;

impl Inner {
    pub fn new(callback: impl FnOnce(i32) + Send + 'static) -> Self {
        let state = IoState::Submitted;
        let callback = Some(Box::new(callback) as _);
        let waker = None;
        Self {
            state,
            callback,
            waker,
        }
    }

    pub fn complete(&mut self, retval: i32) {
        match self.state {
            IoState::Submitted | IoState::Cancelling => {
                self.state = IoState::Completed(retval);
            }
            _ => {
                unreachable!("cannot do complete twice or after cancelled");
            }
        }

        let callback = self.callback.take().unwrap();
        (callback)(retval);
    }

    pub fn cancel(&mut self) {
        todo!("implement cancel in future")
    }

    pub fn retval(&self) -> Option<i32> {
        match self.state {
            IoState::Completed(retval) => Some(retval),
            _ => None,
        }
    }

    pub fn state(&self) -> IoState {
        self.state
    }
}
