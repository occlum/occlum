use self::join::{JoinState, OutputHandle};
use self::task::TaskBuilder;
use crate::executor::EXECUTOR;
use crate::prelude::*;

pub use self::id::TaskId;
pub use self::join::JoinHandle;
pub use self::locals::LocalKey;
pub use self::task::Task;
pub use self::tirqs::Tirqs;

pub(crate) use self::locals::LocalsMap;

pub mod current;
mod id;
mod join;
mod locals;
mod task;
mod tirqs;

pub fn spawn<T: Send + 'static>(future: impl Future<Output = T> + 'static + Send) -> JoinHandle<T> {
    SpawnOptions::new(future).spawn()
}

pub fn block_on<T: Send + 'static>(future: impl Future<Output = T> + 'static + Send) -> T {
    #[cfg(any(test, feature = "auto_run"))]
    init_runner_threads();

    let output_slot: Arc<Mutex<Option<T>>> = Arc::new(Mutex::new(None));
    let completed = Arc::new(AtomicBool::new(false));

    let future = {
        let output_slot = output_slot.clone();
        let completed = completed.clone();

        async move {
            let output = future.await;

            let mut output_slot = output_slot.lock();
            *output_slot = Some(output);
            completed.store(true, Ordering::Release);
        }
    };

    let task = TaskBuilder::new(future).build();
    EXECUTOR.accept_task(task);
    while !completed.load(Ordering::Acquire) {}

    let mut output_slot = output_slot.lock();
    output_slot.take().unwrap()
}

#[cfg(any(test, feature = "auto_run"))]
fn init_runner_threads() {
    use std::sync::Once;
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        // Run test with log:
        // RUST_LOG=trace cargo test -- --nocapture
        let _ = env_logger::builder().is_test(true).try_init().unwrap();

        std::thread::spawn(|| {
            crate::time::run_timer_wheel_thread();
        });

        for _ in 0..crate::executor::num_vcpus() {
            std::thread::spawn(|| {
                let this_vcpu = crate::executor::new_vcpu();
                crate::executor::run_tasks(this_vcpu);
            });
        }
    });
}

pub struct SpawnOptions<T> {
    raw_future: Option<BoxFuture<'static, T>>,
}

impl<T: Send + 'static> SpawnOptions<T> {
    pub fn new(future: impl Future<Output = T> + 'static + Send) -> Self {
        Self {
            raw_future: Some(future.boxed()),
        }
    }

    pub fn spawn(&mut self) -> JoinHandle<T> {
        #[cfg(any(test, feature = "auto_run"))]
        init_runner_threads();

        let state = Arc::new(Mutex::new(JoinState::new()));
        let output_handle = OutputHandle::new(&state);

        let future = {
            let raw_future = self.raw_future.take().unwrap();
            async move {
                let output = raw_future.await;
                output_handle.set(output);
            }
        };
        let task = TaskBuilder::new(future).build();
        let join_handle = JoinHandle::new(state, task.clone());

        EXECUTOR.accept_task(task);
        join_handle
    }
}
