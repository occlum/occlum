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
