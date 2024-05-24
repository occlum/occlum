use core::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize};
use std::{collections::HashMap, thread::current};

use crate::util::sync::Mutex;
use alloc::{sync::Arc, vec::Vec};
use atomic::Ordering;
use io_uring_callback::{Builder, IoUring};
use keyable_arc::KeyableArc;

use crate::config::LIBOS_CONFIG;

// The number of sockets to reach the network bandwidth threshold of one io_uring instance
const SOCKET_THRESHOLD_PER_URING: u32 = 1;

lazy_static::lazy_static! {
    pub static ref MULTITON: UringSet = {
        let uring_set = UringSet::new();
        uring_set
    };

    pub static ref ENABLE_URING: AtomicBool = AtomicBool::new(LIBOS_CONFIG.feature.io_uring > 0);

    // Four uring instances are sufficient to reach the network bandwidth threshold of host kernel.
    pub static ref URING_LIMIT: AtomicUsize = {
        let uring_limit = LIBOS_CONFIG.feature.io_uring;
        assert!(uring_limit <= 16, "io_uring limit must not exceed 16");
        AtomicUsize::new(uring_limit as usize)
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
    running_uring_num: AtomicU32,
}

impl UringSet {
    pub fn new() -> Self {
        let urings = Mutex::new(HashMap::new());
        let running_uring_num = AtomicU32::new(0);
        Self {
            urings,
            running_uring_num,
        }
    }

    pub fn poll_completions(&self) {
        let mut guard = self.urings.lock();
        let uring_limit = URING_LIMIT.load(Ordering::Relaxed) as u32;

        for _ in 0..uring_limit {
            let uring: KeyableArc<IoUring> = Arc::new(
                Builder::new()
                    .setup_sqpoll(500 /* ms */)
                    .build(256)
                    .unwrap(),
            )
            .into();
            let mut state = UringState::default();
            state.enable_poll(uring.clone().into());

            guard.insert(uring.clone(), state);
            self.running_uring_num.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn get_uring(&self) -> Arc<IoUring> {
        let mut map = self.urings.lock();
        let running_uring_num = self.running_uring_num.load(Ordering::Relaxed);
        let uring_limit = URING_LIMIT.load(Ordering::Relaxed) as u32;
        assert!(running_uring_num <= uring_limit);

        let init_stage = running_uring_num < uring_limit;

        // Construct an io_uring instance and initiate a polling thread
        if init_stage {
            let should_build_uring = {
                // Sum registered socket
                let total_socket_num = map
                    .values()
                    .fold(0, |acc, state| acc + state.registered_num);
                // Determine the number of available io_uring
                let uring_num = (total_socket_num / SOCKET_THRESHOLD_PER_URING) + 1;

                running_uring_num < uring_num
            };

            if should_build_uring {
                let uring: KeyableArc<IoUring> = Arc::new(
                    Builder::new()
                        .setup_sqpoll(500 /* ms */)
                        .build(256)
                        .unwrap(),
                )
                .into();
                let mut state = UringState::default();
                state.register_one_socket();
                state.enable_poll(uring.clone().into());

                map.insert(uring.clone(), state);
                self.running_uring_num.fetch_add(1, Ordering::Relaxed);
                return uring.into();
            }
        }

        // Link the file to the io_uring instance with the least load.
        let (mut uring, mut state) = map
            .iter_mut()
            .min_by_key(|(_, state)| state.registered_num)
            .unwrap();

        // Re-select io_uring instance with least task load
        if !init_stage {
            let min_registered_num = state.registered_num;
            (uring, state) = map
                .iter_mut()
                .filter(|(_, state)| state.registered_num == min_registered_num)
                .min_by_key(|(uring, _)| uring.task_load())
                .unwrap();
        } else {
            // At the initial stage, without constructing additional io_uring instances,
            // there exists a singular io_uring which has the minimum number of registered sockets.
        }

        // Update io_uring instance states
        state.register_one_socket();
        assert!(state.is_enable_poll);

        uring.clone().into()
    }

    pub fn disattach_uring(&self, fd: usize, uring: Arc<IoUring>) {
        let uring: KeyableArc<IoUring> = uring.into();
        let mut map = self.urings.lock();
        let mut state = map.get_mut(&uring).unwrap();
        state.unregister_one_socket();
        drop(map);

        uring.disattach_fd(fd);
    }
}
