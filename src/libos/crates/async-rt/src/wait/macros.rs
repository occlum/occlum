use super::{Waiter, WaiterQueue};

/// A waiter queue-coordinated loop.
///
/// Just like a normal loop, except that a waiter queue (as well as a waiter)
/// is used to avoid busy loop. The user code of the loop body can use
/// `break`, `continue`, or `return` as it would do in a normal loop.
///
/// However, there are restrictions on the return value, the user code
/// must return a `Result` when it `break` or `return`. If the user code
/// doesn't use `break` and the waiter_loop isn't the return value, the user
/// code need to use explicit annotation to tell compiler the type of `Result`.
///
/// ```
/// use async_rt::wait::WaiterQueue;
///
/// // Init runtime if not enable "auto_run" feature.
/// async_rt::config::set_parallelism(2);
/// for _ in 0..async_rt::executor::parallelism() {
///     std::thread::spawn(|| {
///         async_rt::executor::run_tasks();
///     });
/// }
///
/// async_rt::task::block_on(async {
///     let waiter_queue = WaiterQueue::new();
///
///     async_rt::waiter_loop!(&waiter_queue, {
///         // do something in each attempt
///         return Ok(());
///     })
/// });
/// ```
///
/// Between each rounds of loop (e.g., via `continue`), the execution will
/// pause until the waiter queue given to the macro call wakes up the waiter
/// associated with this loop.
///
/// This macro is more preferable than using Waiter and WaiterQueue directly.
/// Not only because the macro is more easy to use, but also because it is
/// much harder to be misuse.
///
/// The waiter_loop also support timeout. if user privide a timeout, this loop
/// will be forced to exit when the timeout expired. When the the timeout expired,
/// the loop will break, user should deal with the timeout case by themselves.
/// The timeout argument should be an Option of Duration, e.g., `Option<Duration>`
/// `Option<&mut Duration>`.
///
/// ```
/// # use async_rt::wait::WaiterQueue;
/// #
/// # // Init runtime if not enable "auto_run" feature.
/// # async_rt::config::set_parallelism(2);
/// # for _ in 0..async_rt::executor::parallelism() {
/// #     std::thread::spawn(|| {
/// #         async_rt::executor::run_tasks();
/// #     });
/// # }
/// #
/// # async_rt::task::block_on(async {
/// #     let waiter_queue = WaiterQueue::new();
/// #
///     let mut timeout = Some(std::time::Duration::from_millis(10));
///     async_rt::waiter_loop!(&waiter_queue, timeout, {
///         // do something in each attempt
///         return Ok(());
///     })
///     .map_err(|e| {
///         println!("reach timeout");
///         e
///     })
/// # });
/// ```
#[macro_export]
macro_rules! waiter_loop {
    ($waiter_queue:expr, $loop_body:block) => {{
        let mut timeout = None::<core::time::Duration>;
        $crate::waiter_loop!($waiter_queue, timeout, $loop_body)
    }};
    ($waiter_queue:expr, $timeout:expr, $loop_body:block) => {{
        use core::borrow::BorrowMut;
        use $crate::wait::{AutoWaiter, WaiterQueue};

        let waiter_queue: &WaiterQueue = $waiter_queue;
        let mut auto_waiter = AutoWaiter::new(waiter_queue);
        let mut loop_count: u32 = 0;

        // The loop below may seem a bit of complicated since we want
        // achieve the following three goals simultaneously:
        //
        // 1. The loop_body can only appear once after the expansion of the macro.
        //
        // 2. We do not create the waiter on the first attempt of the loop. This
        // saves a heap allocation and other initialization if we are lucky with
        // the first attempt.
        //
        // 3. Use the combo of waiter and waiter queue properly so that we do
        // not miss any interesting events.
        loop {
            match loop_count {
                0 => {
                    // For the first attempt, we jump directly to the loop body
                    loop_count += 1;
                }
                1 => {
                    // For the second attempt, we init the waiter
                    auto_waiter.waiter();
                    loop_count += 1;
                }
                2 => {
                    // For the third attempt and beyond, we will wait
                    let waiter = auto_waiter.waiter();
                    let wait_res = if let Some(timeout) = $timeout.as_mut() {
                        // Create a temp mut reference rather than passing timeout
                        // directly. This avoids losing the ownership.
                        let timeout: &mut core::time::Duration = (*timeout).borrow_mut();
                        waiter.wait_timeout(Some(timeout)).await
                    } else {
                        waiter.wait().await
                    };
                    // If the timeout expires or the task gets interrupted, exit loop.
                    if let Err(e) = wait_res {
                        break Err(e);
                    }
                    // Prepare the waiter so that we can try the loop body again
                    waiter.reset();
                }
                _ => unreachable!(),
            };

            {
                $loop_body
            }
        }
    }};
}

/// Waiter that will be automatically created on first use and automatically
/// dequeued from its waiter queue on drop.
pub struct AutoWaiter<'a> {
    waiter_queue: &'a WaiterQueue,
    waiter: Option<Waiter>,
}

impl<'a> AutoWaiter<'a> {
    pub fn new(waiter_queue: &'a WaiterQueue) -> Self {
        Self {
            waiter_queue,
            waiter: None,
        }
    }

    pub fn waiter(&mut self) -> &Waiter {
        if self.waiter.is_none() {
            let mut waiter = Waiter::new();
            self.waiter_queue.enqueue(&mut waiter);
            self.waiter = Some(waiter);
        }
        self.waiter.as_ref().unwrap()
    }
}

impl<'a> Drop for AutoWaiter<'a> {
    fn drop(&mut self) {
        if let Some(waiter) = self.waiter.as_mut() {
            self.waiter_queue.dequeue(waiter);
        }
    }
}
