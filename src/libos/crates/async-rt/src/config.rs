use crate::prelude::*;

/// Set the max number of threads that run the executor singleton.
///
/// This function must be called before using the executor (e.g., `crate::task::spawn`)
/// to take effect.
pub fn set_parallelism(parallelism: u32) {
    CONFIG.set_parallelism(parallelism);
}

pub(crate) struct Config {
    inner: Mutex<Inner>,
}

struct Inner {
    parallelism: u32,
}

impl Config {
    pub fn new() -> Self {
        let inner = Inner { parallelism: 1 };
        Self {
            inner: Mutex::new(inner),
        }
    }

    pub fn set_parallelism(&self, parallelism: u32) {
        assert!(parallelism > 0);
        let mut inner = self.inner.lock();
        inner.parallelism = parallelism;
    }

    pub fn parallelism(&self) -> u32 {
        let inner = self.inner.lock();
        inner.parallelism
    }
}

lazy_static! {
    pub(crate) static ref CONFIG: Config = Config::new();
}
