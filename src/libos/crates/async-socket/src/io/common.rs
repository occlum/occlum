#[cfg(feature = "sgx")]
use std::prelude::v1::*;

use atomic::{Atomic, Ordering};

use crate::io::IoUringProvider;
use crate::poll::{Events, Pollee};

// The common part of the socket's sender and receiver.
pub struct Common<P: IoUringProvider> {
    fd: i32,
    pollee: Pollee,
    error: Atomic<Option<i32>>,
    phantom_data: std::marker::PhantomData<P>,
}

impl<P: IoUringProvider> Common<P> {
    pub fn new() -> Self {
        #[cfg(not(feature = "sgx"))]
        let fd = unsafe { libc::socket(libc::AF_INET, libc::SOCK_STREAM, 0) };
        #[cfg(feature = "sgx")]
        let fd = unsafe { libc::ocall::socket(libc::AF_INET, libc::SOCK_STREAM, 0) };
        assert!(fd >= 0);
        let pollee = Pollee::new(Events::empty());
        let error = Atomic::new(None);
        Self {
            fd,
            pollee,
            error,
            phantom_data: std::marker::PhantomData,
        }
    }

    pub fn new_with_fd(fd: i32) -> Self {
        assert!(fd >= 0);
        let pollee = Pollee::new(Events::empty());
        let error = Atomic::new(None);
        Self {
            fd,
            pollee,
            error,
            phantom_data: std::marker::PhantomData,
        }
    }

    pub fn error(&self) -> Option<i32> {
        self.error.load(Ordering::Acquire)
    }

    /// Set error.
    ///
    /// The value must be negative.
    pub fn set_error(&self, error: i32) {
        debug_assert!(error < 0);
        self.error.store(Some(error), Ordering::Release);
    }

    pub fn io_uring(&self) -> P::Instance {
        P::get_instance()
    }

    pub fn fd(&self) -> i32 {
        self.fd
    }

    pub fn pollee(&self) -> &Pollee {
        &self.pollee
    }
}

impl<P: IoUringProvider> Drop for Common<P> {
    fn drop(&mut self) {
        unsafe {
            #[cfg(not(feature = "sgx"))]
            libc::close(self.fd);
            #[cfg(feature = "sgx")]
            libc::ocall::close(self.fd);
        }
    }
}
