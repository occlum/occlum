use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

use keyable_arc::KeyableArc;
use object_id::ObjectId;

use super::{EventCounter, Events};
use crate::prelude::*;

/// A pollee maintains a set of active events, which can be polled and
/// monitored with pollers or subscribers.
pub struct Pollee {
    inner: Arc<PolleeInner>,
}

struct PolleeInner {
    // A table that maintains all interesting pollers
    pollers: Mutex<HashMap<KeyableArc<PollerInner>, Events>>,
    // For efficient manipulation, we use AtomicU32 instead of Atomic<Events>
    events: AtomicU32,
    // To reduce lock contentions, we maintain a counter for the size of the table
    num_pollers: AtomicUsize,
}

impl Pollee {
    /// Creates a new instance of pollee.
    pub fn new(init_events: Events) -> Self {
        let inner = PolleeInner {
            pollers: Mutex::new(HashMap::new()),
            events: AtomicU32::new(init_events.bits()),
            num_pollers: AtomicUsize::new(0),
        };
        Self {
            inner: Arc::new(inner),
        }
    }

    /// Returns the current events of the pollee given an event mask.
    ///
    /// If no interesting events are polled and a poller is provided, then
    /// the poller will start monitoring the pollee and receive event
    /// notification once the pollee gets any interesting events.
    ///
    /// This operation is _atomic_ in the sense that either some interesting
    /// events are returned or the poller is registered (if a poller is provided).
    pub fn poll_by(&self, mask: Events, poller: Option<&mut Poller>) -> Events {
        let mask = mask | Events::ALWAYS_POLL;

        // Fast path: return events immediately
        if poller.is_none() {
            let revents = self.events() & mask;
            return revents;
        }

        // Slow path: connect the pollee with the poller
        let poller = poller.unwrap();

        let mut pollers = self.inner.pollers.lock();
        let is_new = pollers.insert(poller.inner.clone(), mask).is_none();
        if is_new {
            let mut pollees = poller.inner.pollees.lock();
            pollees.push(Arc::downgrade(&self.inner));

            self.inner.num_pollers.fetch_add(1, Ordering::Release);
        }
        drop(pollers);

        // It is important to check events again to handle race conditions
        let revents = self.events() & mask;
        revents
    }

    /// Add some events to the pollee's state.
    ///
    /// This method wakes up all registered pollers that are interested in
    /// the added events.
    pub fn add_events(&self, events: Events) {
        self.inner.events.fetch_or(events.bits(), Ordering::Release);

        // Fast path
        if self.inner.num_pollers.load(Ordering::Relaxed) == 0 {
            return;
        }

        // Slow path: broadcast the new events to all pollers
        let pollers = self.inner.pollers.lock();
        pollers
            .iter()
            .filter(|(_, mask)| mask.intersects(events))
            .for_each(|(poller, _)| poller.event_counter.write());
    }

    /// Remove some events from the pollee's state.
    ///
    /// This method will not wake up registered pollers even when
    /// the pollee still has some interesting events to the pollers.
    pub fn del_events(&self, events: Events) {
        self.inner
            .events
            .fetch_and(!events.bits(), Ordering::Release);
    }

    /// Reset theh pollee's state.
    ///
    /// Reset means removing all events on the pollee.
    pub fn reset_events(&self) {
        self.inner
            .events
            .fetch_and(!Events::all().bits(), Ordering::Release);
    }

    fn events(&self) -> Events {
        let event_bits = self.inner.events.load(Ordering::Relaxed);
        unsafe { Events::from_bits_unchecked(event_bits) }
    }
}

impl std::fmt::Debug for Pollee {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pollee")
            .field("events", &self.events())
            .field(
                "num_pollers",
                &self.inner.num_pollers.load(Ordering::Relaxed),
            )
            .field("pollers", &"..")
            .finish()
    }
}

/// A poller gets notified when its associated pollees have interesting events.
#[derive(PartialEq, Eq, Hash)]
pub struct Poller {
    inner: KeyableArc<PollerInner>,
}

struct PollerInner {
    // Use event counter to wait or wake up a poller
    event_counter: EventCounter,
    // All pollees that are interesting to this poller
    pollees: Mutex<Vec<Weak<PolleeInner>>>,
}

impl Poller {
    pub fn new() -> Self {
        let inner = PollerInner {
            event_counter: EventCounter::new(),
            pollees: Mutex::new(Vec::with_capacity(1)),
        };
        Self {
            inner: KeyableArc::new(inner),
        }
    }

    pub async fn wait(&self) {
        self.inner.event_counter.read().await;
    }
}

impl Drop for Poller {
    fn drop(&mut self) {
        let mut pollees = self.inner.pollees.lock();
        for weak_pollee in pollees.drain(..) {
            if let Some(pollee) = weak_pollee.upgrade() {
                let mut pollers = pollee.pollers.lock();
                pollers.remove(&self.inner);
                drop(pollers);

                pollee.num_pollers.fetch_sub(1, Ordering::Relaxed);
            }
        }
    }
}
