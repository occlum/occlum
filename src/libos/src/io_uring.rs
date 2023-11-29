use core::sync::atomic::{AtomicU32, AtomicUsize};
use std::{collections::HashMap, thread::current};

use alloc::{sync::Arc, vec::Vec};
use io_uring_callback::{Builder, IoUring};
use keyable_arc::KeyableArc;
use spin::{Mutex, RwLock};

// Four uring instances are sufficient to reach the network bandwidth threshold.
const URING_NUM_LIMIT: usize = 4;

lazy_static::lazy_static! {
    pub static ref MULTITON: UringSet = {
        let uring_set = UringSet::new();
        uring_set
    };
}

#[derive(Clone, Copy, Default)]
struct UringState {
    registered_num: u32,
    is_enable_poll: bool, // CQE polling thread
}

impl UringState {
    fn register_one_socket(&mut self) {
        self.registered_num += 1;
    }

    fn unregister_one_socket(&mut self) {
        self.registered_num -= 1;
    }

    fn enable_poll(&mut self, uring: Arc<IoUring>) {
        if !self.is_enable_poll {
            self.is_enable_poll = true;
            std::thread::spawn(move || loop {
                let min_complete = 1;
                let polling_retries = 10000;
                uring.poll_completions(min_complete, polling_retries);
            });
        }
    }
}

pub struct UringSet {
    urings: Mutex<HashMap<KeyableArc<IoUring>, UringState>>,
}

impl UringSet {
    pub fn new() -> Self {
        let urings = Mutex::new(HashMap::new());

        let mut guard = urings.lock();
        for _ in 0..URING_NUM_LIMIT {
            let uring: KeyableArc<IoUring> = Arc::new(
                Builder::new()
                    .setup_sqpoll(500 /* ms */)
                    .build(256)
                    .unwrap(),
            )
            .into();
            guard.insert(uring, UringState::default());
        }

        drop(guard);

        Self { urings }
    }

    pub fn poll_completions(&self) {
        let mut guard = self.urings.lock();

        for (uring, state) in guard.iter_mut() {
            state.enable_poll(uring.clone().into())
        }
    }

    pub fn get_uring(&self) -> Arc<IoUring> {
        let mut map = self.urings.lock();
        let (uring, state) = map
            .iter_mut()
            .min_by_key(|(_, &mut state)| state.registered_num)
            .unwrap();
        // Update states
        state.register_one_socket();
        if !state.is_enable_poll {
            state.enable_poll(uring.clone().into());
        }

        uring.clone().into()
    }

    pub fn free_uring(&self, uring: Arc<IoUring>) {
        let uring: KeyableArc<IoUring> = uring.into();
        let mut map = self.urings.lock();
        let mut state = map.get_mut(&uring).unwrap();

        state.unregister_one_socket();
    }
}
