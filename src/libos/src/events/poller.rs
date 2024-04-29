use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Weak;
use std::time::Duration;

use super::{EdgeSync, Notifier, Observer, Waiter};
use crate::fs::{IoEvents, IoNotifier};
use crate::prelude::*;

/// A pollee maintains a set of active events, which can be polled with
/// pollers or be monitored with observers.
pub struct Pollee {
    inner: Arc<PolleeInner>,
}

struct PolleeInner {
    // A table that maintains all interesting pollers
    pollers: IoNotifier,
    // For efficient manipulation, we use AtomicU32 instead of Atomic<Events>
    events: AtomicU32,
}

impl Pollee {
    /// Creates a new instance of pollee.
    pub fn new(init_events: IoEvents) -> Self {
        let inner = PolleeInner {
            pollers: Notifier::new(),
            events: AtomicU32::new(init_events.bits()),
        };
        Self {
            inner: Arc::new(inner),
        }
    }

    pub fn notifier(&self) -> &IoNotifier {
        &self.inner.pollers
    }

    /// Returns the current events of the pollee given an event mask.
    ///
    /// If no interesting events are polled and a poller is provided, then
    /// the poller will start monitoring the pollee and receive event
    /// notification once the pollee gets any interesting events.
    ///
    /// This operation is _atomic_ in the sense that either some interesting
    /// events are returned or the poller is registered (if a poller is provided).
    pub fn poll(&self, mask: IoEvents, poller: Option<&Poller>) -> IoEvents {
        let mask = mask | IoEvents::ALWAYS_POLL;

        // Fast path: return events immediately
        if poller.is_none() {
            let revents = self.events() & mask;
            return revents;
        }

        // Slow path: connect the pollee with the poller
        self.connect_poller(mask, poller.unwrap());

        // It is important to check events again to handle race conditions
        self.events() & mask
    }

    pub fn connect_poller(&self, mask: IoEvents, poller: &Poller) {
        self.register_observer(poller.observer(), mask);

        let mut pollees = poller.inner.pollees.lock();
        pollees.push(Arc::downgrade(&self.inner).into());
    }

    /// Add some events to the pollee's state.
    ///
    /// This method wakes up all registered pollers that are interested in
    /// the added events.
    pub fn add_events(&self, events: IoEvents) {
        self.inner.events.fetch_or(events.bits(), Ordering::Release);
        self.inner.pollers.broadcast(&events);
    }

    /// Remove some events from the pollee's state.
    ///
    /// This method will not wake up registered pollers even when
    /// the pollee still has some interesting events to the pollers.
    pub fn del_events(&self, events: IoEvents) {
        self.inner
            .events
            .fetch_and(!events.bits(), Ordering::Release);
    }

    /// Reset the pollee's state.
    ///
    /// Reset means removing all events on the pollee.
    pub fn reset_events(&self) {
        self.inner
            .events
            .fetch_and(!IoEvents::all().bits(), Ordering::Release);
    }

    /// Register an event observer.
    ///
    /// A registered observer will get notified (through its `on_events` method)
    /// every time new events specified by the `masks` argument happen on the
    /// pollee (through the `add_events` method).
    ///
    /// If the given observer has already been registered, then its registered
    /// event mask will be updated.
    ///
    /// Note that the observer will always get notified of the events in
    /// `Events::ALWAYS_POLL` regardless of the value of `masks`.
    ///
    /// # Memory leakage
    ///
    /// Since an `Arc` for each observer is kept internally by a pollee,
    /// it is important for the user to call the `unregister_observer` method
    /// when the observer is no longer interested in the pollee. Otherwise,
    /// the observer will not be dropped.
    pub fn register_observer(&self, observer: Weak<dyn Observer<IoEvents>>, mask: IoEvents) {
        let mask = mask | IoEvents::ALWAYS_POLL;
        self.inner.pollers.register(observer, Some(mask), None)
    }

    /// Unregister an event observer.
    ///
    /// If such an observer is found, then the registered observer will be
    /// removed from the pollee and returned as the return value. Otherwise,
    /// a `None` will be returned.
    pub fn unregister_observer(&self, observer: &Weak<dyn Observer<IoEvents>>) {
        self.inner.pollers.unregister(observer)
    }

    fn events(&self) -> IoEvents {
        let event_bits = self.inner.events.load(Ordering::Relaxed);
        unsafe { IoEvents::from_bits_unchecked(event_bits) }
    }
}

impl std::fmt::Debug for Pollee {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pollee")
            .field("events", &self.events())
            .field("pollers", &"..")
            .finish()
    }
}

/// A poller gets notified when its associated pollees have interesting events.
pub struct Poller {
    inner: Arc<PollerInner>,
}

struct PollerInner {
    // Use event counter to wait or wake up a poller
    waiter: Waiter<EdgeSync>,
    // All pollees that are interesting to this poller
    pollees: Mutex<Vec<Weak<PolleeInner>>>,
}

unsafe impl Send for PollerInner {}
unsafe impl Sync for PollerInner {}

impl Poller {
    /// Constructs a new `Poller`.
    pub fn new() -> Self {
        let inner = PollerInner {
            waiter: Waiter::<EdgeSync>::new(),
            pollees: Mutex::new(Vec::new()),
        };
        Self {
            inner: Arc::new(inner),
        }
    }

    /// Wait until there are any interesting events happen since last `wait`.
    pub fn wait(&self) -> Result<()> {
        self.inner.waiter.wait(None)
    }

    /// Wait until there are any interesting events happen since last `wait`, or reach timeout.
    pub fn wait_timeout(&self, timeout: Option<&mut Duration>) -> Result<()> {
        self.inner.waiter.wait_mut(timeout)
    }

    pub fn observer(&self) -> Weak<dyn Observer<IoEvents>> {
        Arc::downgrade(&self.inner) as _
    }
}

impl Observer<IoEvents> for PollerInner {
    fn on_event(
        &self,
        _event: &IoEvents,
        _metadata: &Option<Weak<dyn core::any::Any + Send + Sync>>,
    ) -> () {
        self.waiter.waker().wake();
    }
}

impl Drop for Poller {
    fn drop(&mut self) {
        let mut pollees = self.inner.pollees.lock();
        if pollees.len() == 0 {
            return;
        }

        let self_observer = self.observer();
        for weak_pollee in pollees.drain(..) {
            if let Some(pollee) = weak_pollee.upgrade() {
                pollee.pollers.unregister(&self_observer);
            }
        }
    }
}
