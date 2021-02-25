use super::{Waiter, WaiterQueue};

/// A waiter queue-coordinated loop.
///
/// Just like a normal loop, except that a waiter queue (as well as a waiter)
/// is used to avoid busy loop. The user code of the loop body can use
/// `break`, `continue`, or `return` as it would do in a normal loop.
///
/// Between each rounds of loop (e.g., via `continue`), the execution will
/// pause until the waiter queue given to the macro call wakes up the waiter
/// associated with this loop.
///
/// This macro is more preferable than using Waiter and WaiterQueue directly.
/// Not only because the macro is more easy to use, but also because it is
/// much harder to be misuse.
#[macro_export]
macro_rules! waiter_loop {
    ($waiter_queue:expr, $loop_body:block) => {{
        use $crate::wait::{AutoWaiter, Waiter, WaiterQueue};

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
                    // Wait until being woken by the waiter queue
                    waiter.wait().await;
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
