use crate::prelude::*;

/// Set the max number of threads that run the executor singleton.
///
/// This function must be called before using the executor (e.g., `crate::task::spawn`)
/// to take effect.
pub fn set_parallelism(parallelism: u32) {
    CONFIG.set_parallelism(parallelism);
}

/// Set a callback function that will be invoked right before a task will be
/// scheduled to run.
///
/// Similar to `set_parallelism`, this function must be called before using
/// the executor to take effect. This callback can help integrate into the
/// async runtime a reactor that processes I/O completions and wakes up tasks.
pub fn set_sched_callback(f: impl Fn() + Send + Sync + 'static) {
    CONFIG.set_sched_callback(f);
}

pub(crate) struct Config {
    inner: Mutex<Inner>,
}

struct Inner {
    parallelism: u32,
    sched_callback: Option<Box<dyn Fn() + Send + Sync + 'static>>,
}

impl Config {
    pub fn new() -> Self {
        let inner = Inner {
            parallelism: 1,
            sched_callback: None,
        };
        Self {
            inner: Mutex::new(inner),
        }
    }

    pub fn set_parallelism(&self, parallelism: u32) {
        assert!(parallelism > 0);
        let mut inner = self.inner.lock();
        inner.parallelism = parallelism;
    }

    pub fn set_sched_callback(&self, f: impl Fn() + Send + Sync + 'static) {
        let boxed_f = Box::new(f) as Box<dyn Fn() + Send + Sync + 'static>;
        let mut inner = self.inner.lock();
        inner.sched_callback = Some(boxed_f);
    }

    pub fn parallelism(&self) -> u32 {
        let inner = self.inner.lock();
        inner.parallelism
    }

    pub fn take_sched_callback(&self) -> Box<dyn Fn() + Send + Sync + 'static> {
        let mut inner = self.inner.lock();
        inner.sched_callback.take().unwrap_or_else(|| {
            Box::new(|| { /* dummy callback */ }) as Box<dyn Fn() + Send + Sync + 'static>
        })
    }
}

lazy_static! {
    pub(crate) static ref CONFIG: Config = Config::new();
}
