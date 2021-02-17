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
        let mut is_first = true;

        // The main loop cannot be written in the most natural way since we want
        // achieve the following two goals simultaneously:
        //
        // 1. The loop_body can only appear once after the expansion of the macro.
        //
        // 2. We do not create the waiter on the first attempt of the loop. This
        // saves a heap allocation and other initialization if we are lucky with
        // the first attempt.
        loop {
            if is_first {
                is_first = false;
            } else {
                let waiter = auto_waiter.waiter();
                // Wait until being woken by the waiter queue
                waiter.wait().await;

                // Prepare the waiter so that we can try the loop body again
                waiter.reset();
            }

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
