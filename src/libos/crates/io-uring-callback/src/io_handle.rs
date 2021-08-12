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
#[derive(Debug)]
pub struct IoHandle(pub(crate) Arc<IoToken>);

/// The state of an I/O request represented by an [`IoHandle`].
/// If a request is in `Processed` or `Cancelled` state, means that the request is completed.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum IoState {
    /// The I/O request has been submitted.
    Submitted,
    /// The I/O request has been processed by the kernel and returns a value.
    Processed(i32),
    /// The I/O request is being cancelled.
    Cancelling,
    /// The I/O request has been cancelled by the kernel.
    Cancelled,
}

const CANCEL_RETVAL: i32 = -libc::ECANCELED;

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
}

impl Unpin for IoHandle {}

impl Future for IoHandle {
    type Output = i32;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut inner = self.0.inner.lock().unwrap();
        match inner.state {
            IoState::Processed(retval) => Poll::Ready(retval),
            IoState::Cancelled => Poll::Ready(CANCEL_RETVAL),
            IoState::Submitted | IoState::Cancelling => {
                inner.waker = Some(cx.waker().clone());
                Poll::Pending
            }
        }
    }
}

impl Drop for IoHandle {
    fn drop(&mut self) {
        // The user cannot drop a handle if the request isn't completed.
        assert!(matches!(self.state(), IoState::Processed(_) | IoState::Cancelled));
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
    pub fn new(completion_callback: impl FnOnce(i32) + Send + 'static, token_key: u64) -> Self {
        let inner = Mutex::new(Inner::new(completion_callback, token_key));
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
        let callback = inner.complete(retval);
        // Must release the lock before invoking the callback function.
        // This avoids any deadlock if the IoHandle is accessed inside the callback by
        // user.
        drop(inner);

        (callback)(retval);
    }

    /// Change the state from submited to cancelling.
    /// If transition succeeds, return the token_key for following cancel operation.
    pub fn transit_to_cancelling(&self) -> Result<u64, ()> {
        let mut inner = self.inner.lock().unwrap();
        inner.transit_to_cancelling()
    }
}

impl std::fmt::Debug for IoToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IoToken")
            .field("state", &self.state())
            .finish()
    }
}

struct Inner {
    state: IoState,
    completion_callback: Option<Callback>,
    waker: Option<Waker>,
    token_key: u64,
}

type Callback = Box<dyn FnOnce(i32) + Send + 'static>;

impl Inner {
    pub fn new(completion_callback: impl FnOnce(i32) + Send + 'static, token_key: u64) -> Self {
        let state = IoState::Submitted;
        let completion_callback = Some(Box::new(completion_callback) as _);
        let waker = None;
        Self {
            state,
            completion_callback,
            waker,
            token_key,
        }
    }

    pub fn complete(&mut self, retval: i32) -> Callback {
        match self.state {
            IoState::Submitted => {
                self.state = IoState::Processed(retval);
            }
            IoState::Cancelling => {
                if retval == CANCEL_RETVAL {
                    // case 1: The request was cancelled successfully.
                    self.state = IoState::Cancelled;
                } else {
                    // case 2.1: The request was cancelled with error.
                    // case 2.2: The request was not actually canceled.
                    self.state = IoState::Processed(retval);
                }
            }
            _ => {
                unreachable!("cannot do complete twice");
            }
        }

        self.completion_callback.take().unwrap()
    }

    pub fn transit_to_cancelling(&mut self) -> Result<u64, ()> {
        match self.state {
            IoState::Submitted => {
                self.state = IoState::Cancelling;
                return Ok(self.token_key);
            }
            _ => {
                return Err(());
            }
        }
    }

    pub fn retval(&self) -> Option<i32> {
        match self.state {
            IoState::Processed(retval) => Some(retval),
            IoState::Cancelled => Some(CANCEL_RETVAL),
            _ => None,
        }
    }

    pub fn state(&self) -> IoState {
        self.state
    }
}
