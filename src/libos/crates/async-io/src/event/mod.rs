//! The event mechanism for async I/O.
//!
//! # Overview
//!
//! This module provides four core event primitives for enabling async I/O:
//!
//! * `Events` represents Linux-compatible I/O events (e.g., `POLLIN`, `POLLOUT`, etc.).
//! * `Pollee` is a file-like entity that has a state of _I/O readiness_ (i.e., `Events`).
//! Its name stems from the fact such entities can be _polled_ for its I/O readiness.
//! * `Poller` enables waiting on the readiness of one or multiple `Pollee`s. One can use
//! pollers to implement `select` and `poll` system calls easily.
//! * `Observer` is a more general form of event monitoring than `Poller`.
//! While pollers can allow _waiting_ for events, observers allow _handling_ events in a
//! customized way. One can use observers to implement the `epoll` system call easily.
//!
//! # Usage
//!
//! Here we showcase how the combination of `Events`, `Pollee`, and `Poller` enables
//! writing an async file-like object named `OneItemQueue` easily. `OneItemQueue` is a fixed-size
//! queue that can contain at most one item.
//!
//! ```rust
//! use std::sync::{Arc, Mutex};
//!
//! use async_io::event::{Events, Pollee, Poller};
//!
//! /// A fixed-size queue that can contain at most one item.
//! pub struct OneItemQueue<T> {
//!     slot: Mutex<Option<T>>,
//!     pollee: Pollee,
//! }
//!
//! impl<T> OneItemQueue<T> {
//! /// Construct an empty queue.
//! pub fn new() -> Self {
//!         let empty_slot = Mutex::new(None);
//!         let writable_pollee = Pollee::new(Events::OUT);
//!         Self {
//!             slot: empty_slot,
//!             pollee: writable_pollee,
//!         }
//!     }
//!
//!     /// Push one item into the queue. This method blocks if the queue is full.
//!     pub async fn push(&self, item: T) {
//!         let mut poller = None;
//!         loop {
//!             // Try to push
//!             let mask = Events::OUT; // = writable or the queue is empty
//!             let is_writable = !self.pollee.poll(mask, poller.as_mut()).is_empty();
//!             if is_writable {
//!                 let mut slot = self.slot.lock().unwrap();
//!                 if slot.is_none() {
//!                     *slot = Some(item);
//!
//!                     // Mark the queue as readable yet unwritable
//!                     self.pollee.del_events(Events::OUT);
//!                     self.pollee.add_events(Events::IN);
//!                     return;
//!                 }
//!             }
//!
//!             // Initialize the poller only when necessary
//!             if poller.is_none() {
//!                 poller = Some(Poller::new());
//!             }
//!             poller.as_ref().unwrap().wait().await;
//!         }
//!     }
//!
//!     /// Pop an item from the queue. This method blocks if the queue is empty.
//!     pub async fn pop(&self) -> T {
//!         let mut poller = None;
//!         loop {
//!             // Try to pop
//!             let mask = Events::IN; // = readable or the queue has an item
//!             let is_readable = !self.pollee.poll(mask, poller.as_mut()).is_empty();
//!             if is_readable {
//!                 let mut slot = self.slot.lock().unwrap();
//!                 let item = slot.take();
//!                 if item.is_some() {
//!                     let item = item.unwrap();
//!                     
//!                     // Mark the queue as writable yet unreadable.
//!                     self.pollee.del_events(Events::IN);
//!                     self.pollee.add_events(Events::OUT);
//!                     return item;
//!                 }
//!             }
//!
//!             // Initialize the poller only when necessary
//!             if poller.is_none() {
//!                 poller = Some(Poller::new());
//!             }
//!             poller.as_ref().unwrap().wait().await;
//!         }
//!     }
//!
//!     /// Poll the I/O readiness of the queue for the interesting events specified
//!     /// by the mask.
//!     ///
//!     /// If an poller is provided and the queue is not ready for the interesting events,
//!     /// then the poller will start monitoring the queue and get woken up once interesting
//!     /// events happen on the queue.
//!     ///
//!     /// By providing this method, a poller can now monitor multiple instances of
//!     /// `OneItemQueue`.
//!     pub fn poll(&self, mask: Events, poller: Option<&mut Poller>) -> Events {
//!         self.pollee.poll(mask, poller)
//!     }
//! }
//! ```
//!
//! In addition to the three primitives shown above, one can use `Observer` to
//! capture and handle events. See the docs of `Observer` and `Pollee`'s
//! `register_observer` and `unregister_observer` methods for more details.

mod event_counter;
mod events;
mod observer;
mod poller;

// TODO: fix a bug in Poller::drop

pub use self::event_counter::EventCounter;
pub use self::events::Events;
pub use self::observer::Observer;
pub use self::poller::{Pollee, Poller};
