use alloc::sync::Arc;
use core::future::Future;

use crate::executor::EXECUTOR;
use crate::prelude::*;

pub use self::current::current;
pub use self::id::TaskId;
pub use self::join::JoinHandle;
pub use self::locals::LocalKey;
pub use self::task::Task;

pub(crate) use self::current::{reset_current, set_current};
pub(crate) use self::locals::LocalsMap;

mod current;
mod id;
mod join;
mod locals;
mod task;

pub fn spawn<T: Send + 'static>(future: impl Future<Output = T> + 'static + Send) -> JoinHandle<T> {
    let (join_handle, output_handle) = join::new();
    let future = async move {
        let output = future.await;
        output_handle.set(output);
    };
    let task = Arc::new(Task::new(future));
    EXECUTOR.accept_task(task);
    join_handle
}

pub fn block_on<T: Send + 'static>(future: impl Future<Output = T> + 'static + Send) -> T {
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

    let task = Arc::new(Task::new(future));
    EXECUTOR.accept_task(task);
    while !completed.load(Ordering::Acquire) {}

    let mut output_slot = output_slot.lock();
    output_slot.take().unwrap()
}
