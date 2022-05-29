use crate::prelude::*;
#[cfg(feature = "sgx")]
use std::thread::SgxThread as Thread;
#[cfg(not(feature = "sgx"))]
use std::thread::Thread;

use super::vcpu;

lazy_static! {
    pub static ref PARKS: Parks = {
        let num_vcpus = vcpu::get_total();
        Parks::new(num_vcpus)
    };
}

/// Park, i.e., put the current thread to sleep.
///
/// This method must not be called concurrently.
/// Doing so does not lead any memory safety issues, but
/// may cause a parked thread to sleep forever.
pub fn park() {
    PARKS.park();
}

/// Unpark, i.e., wake up a thread put to sleep by the parker.
///
/// This method can be called concurrently.
pub fn unpark(this_vcpu: usize) {
    PARKS.unpark(this_vcpu);
}

/// Unpark all the vcpu threads.
pub fn unpark_all() {
    PARKS.unpark_all();
}

/// A threads vector for thread parking and unparking.
pub struct Parks {
    threads: Vec<Mutex<Option<Thread>>>,
}

impl Parks {
    /// Initialize Parks
    pub fn new(num_vcpus: u32) -> Self {
        let threads: Vec<_> = (0..num_vcpus).map(|_| Mutex::new(None)).collect();
        Self { threads }
    }

    /// Register current thread
    pub fn register(&self, this_vcpu: usize) {
        assert!(this_vcpu < self.threads.len());

        let mut vcpu_thread = self.threads[this_vcpu].lock();
        vcpu_thread.replace(std::thread::current());
    }

    /// Unregister current thread
    pub fn unregister(&self, this_vcpu: usize) {
        assert!(this_vcpu < self.threads.len());

        let mut vcpu_thread = self.threads[this_vcpu].lock();
        vcpu_thread.take();
    }

    /// Park current thread
    pub fn park(&self) {
        std::thread::park();
    }

    /// Park the thread until timeout (millisecond)
    pub fn park_timeout(&self, duration: core::time::Duration) {
        std::thread::park_timeout(duration);
    }

    /// Unpark the vcpu thread
    pub fn unpark(&self, this_vcpu: usize) {
        assert!(this_vcpu < self.threads.len());

        let vcpu_thread = {
            let thread_opt = self.threads[this_vcpu].lock().clone();
            match thread_opt {
                None => return,
                Some(thread) => thread,
            }
        };
        vcpu_thread.unpark();
    }

    /// Unpark all the vcpu thread
    pub fn unpark_all(&self) {
        for vcpu_id in 0..self.threads.len() {
            self.unpark(vcpu_id);
        }
    }

    fn len(&self) -> usize {
        self.threads.len()
    }
}
