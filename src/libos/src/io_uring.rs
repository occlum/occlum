use io_uring_callback::{Builder, IoState, IoUring, TimeoutFlags, Timespec};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::SgxRwLock;

pub use io_uring_callback::IoUringRef;

lazy_static::lazy_static! {
    pub static ref IO_URING_MANAGER: IoUringManager = IoUringManager::new();
}

// A number of vcpu can share the same io_uring instance.
//
// Currently, based on iperf2 benchmarks, we assign each vcpu a single io_uring instance to achieve best performance.
// However, when there are more than 4 vcpus, the performance will drop. Further investigation is needed.
const NUM_OF_VCPU_SHARING_SINGLE_IO_URING: u32 = 1;

pub struct IoUringManager(SgxRwLock<HashMap<u32, IoUringInstance>>); // Key: io_uring_uid, Value: IoUringInstance

impl IoUringManager {
    fn new() -> Self {
        Self(SgxRwLock::new(HashMap::new()))
    }

    pub fn assign_io_uring_instance_for_vcpu(&self, vcpu_id: u32) {
        let io_uring_uid = Self::get_io_uring_uid(vcpu_id);
        let mut manager = self.0.write().unwrap();
        if manager.get(&io_uring_uid).is_none() {
            let instance = IoUringInstance::new();
            manager.insert(io_uring_uid, instance);
        }
    }

    pub fn get_io_uring_ref(&self, vcpu_id: u32) -> Option<IoUringRef> {
        let io_uring_uid = Self::get_io_uring_uid(vcpu_id);
        let manager = self.0.read().unwrap();
        if let Some(io_uring) = manager.get(&io_uring_uid) {
            Some(io_uring.inner.clone())
        } else {
            None
        }
    }

    pub fn clear(&self) {
        let mut manager = self.0.write().unwrap();
        manager.clear();
    }

    fn get_io_uring_uid(vcpu_id: u32) -> u32 {
        vcpu_id / NUM_OF_VCPU_SHARING_SINGLE_IO_URING
    }
}

pub struct IoUringInstance {
    inner: IoUringRef,
}

impl IoUringInstance {
    pub fn new() -> Self {
        let io_uring = Builder::new()
            .setup_sqpoll(1000 /* ms */)
            .build(256)
            .unwrap();

        let inner = Arc::new(io_uring);

        let inner_copy = inner.clone();

        std::thread::spawn(move || loop {
            let min_complete = 1;
            let polling_retries = 10000;
            let ret = inner_copy.poll_completions(min_complete, polling_retries);
            if ret == 0 {
                break;
            }
        });

        Self { inner }
    }
}
