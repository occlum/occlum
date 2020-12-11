//! The event subsystem.
//!
//! An event can be anything ranging from the exit of a process (interesting
//! to `wait4`) to the arrival of a blocked signal (interesting to `sigwaitinfo`),
//! from the completion of a file operation (interesting to `epoll`) to the change
//! of a file status (interesting to `inotify`).
//!
//! To meet the event-related demands from various subsystems, this event
//! subsystem is designed to provide a set of general-purpose primitives:
//!
//! * `Waiter`, `Waker`, and `WaiterQueue` are primitives to put threads to sleep
//! and later wake them up.
//! * `Event`, `Observer`, and `Notifier` are primitives to handle and broadcast
//! events.
//! * `WaiterQueueObserver` implements the common pattern of waking up threads
//! * once some interesting events happen.

mod event;
mod host_event_fd;
mod notifier;
mod observer;
mod waiter;
mod waiter_queue;
mod waiter_queue_observer;

pub use self::event::{Event, EventFilter};
pub use self::host_event_fd::HostEventFd;
pub use self::notifier::Notifier;
pub use self::observer::Observer;
pub use self::waiter::{Waiter, Waker};
pub use self::waiter_queue::WaiterQueue;
pub use self::waiter_queue_observer::WaiterQueueObserver;

// Just like a normal loop, except that a waiter queue (as well as a waiter)
// is used to avoid busy loop. This macro is more preferable than using
// Waiter and WaiterQueue directly. Not only because the macro is more
// easy to use, but also because it is much harder to be misuse.
#[macro_export]
macro_rules! waiter_loop {
    ($loop_body:block, $waiter_queue:expr) => {
        $crate::waiter_loop!($loop_body, $waiter_queue, None);
    };
    ($loop_body:block, $waiter_queue:expr, $timeout:expr) => {{
        use std::time::Duration;
        use $crate::events::{Waiter, WaiterQueue};
        use $crate::prelude::*;
        use $crate::util::delay::Delay;

        let waiter = Waiter::new();
        let waiter: &Waiter = &waiter;
        let waiter_queue: &WaiterQueue = $waiter_queue;
        let timeout: Option<&Duration> = $timeout;
        let mut timeout: Option<Duration> = timeout.cloned();

        // This ensures that whatever the outcomes of the loop, the waiter is
        // always dequeued from the waiter queue when this method returns.
        // This prevents potential memory leakage.
        let auto_dequeue = Delay::new(|| {
            // When Waiter is used in combination with WaiterQueue, it is always
            // through WaitQueue::dequeue_and_wake_xxx() to wake up the Waiter.
            // So if the Waiter is woken, then we don't need to try dequeuing.
            if !waiter.is_woken() {
                waiter_queue.dequeue(waiter);
            }
        });

        let ret: Result<_> = loop {
            waiter_queue.enqueue(waiter);

            {
                $loop_body
            }

            let ret = waiter.wait_mut(timeout.as_mut());
            if let Err(e) = ret {
                break Err(e);
            }

            waiter.reset();
        };
        ret
    }};
}
