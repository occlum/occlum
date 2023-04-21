use std::borrow::BorrowMut;
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::time::Duration;

use keyable_arc::KeyableArc;
use object_id::ObjectId;

use super::{EventCounter, Events, Observer};
use crate::prelude::*;

/// A pollee maintains a set of active events, which can be polled with
/// pollers or be monitored with observers.
pub struct Pollee {
    inner: Arc<PolleeInner>,
}

struct PolleeInner {
    // A unique ID
    id: ObjectId,
    // A table that maintains all interesting pollers
    pollers: Mutex<HashMap<KeyableArc<dyn Observer>, Events>>,
    // For efficient manipulation, we use AtomicU32 instead of Atomic<Events>
    events: AtomicU32,
    // To reduce lock contentions, we maintain a counter for the size of the table
    num_pollers: AtomicUsize,
}

impl Pollee {
    /// Creates a new instance of pollee.
    pub fn new(init_events: Events) -> Self {
        let inner = PolleeInner {
            id: ObjectId::new(),
            pollers: Mutex::new(HashMap::new()),
            events: AtomicU32::new(init_events.bits()),
            num_pollers: AtomicUsize::new(0),
        };
        Self {
            inner: Arc::new(inner),
        }
    }

    /// Returns the unique ID of this pollee.
    pub fn id(&self) -> &ObjectId {
        &self.inner.id
    }

    /// Returns the current events of the pollee given an event mask.
    ///
    /// If no interesting events are polled and a poller is provided, then
    /// the poller will start monitoring the pollee and receive event
    /// notification once the pollee gets any interesting events.
    ///
    /// This operation is _atomic_ in the sense that either some interesting
    /// events are returned or the poller is registered (if a poller is provided).
    pub fn poll(&self, mask: Events, poller: Option<&Poller>) -> Events {
        let mask = mask | Events::ALWAYS_POLL;

        // Fast path: return events immediately
        if poller.is_none() {
            let revents = self.events() & mask;
            return revents;
        }

        // Slow path: connect the pollee with the poller
        self.connect_poller(mask, poller.unwrap());

        // It is important to check events again to handle race conditions
        let revents = self.events() & mask;
        revents
    }

    pub fn connect_poller(&self, mask: Events, poller: &Poller) {
        let mask = mask | Events::ALWAYS_POLL;

        let mut pollers = self.inner.pollers.lock();
        let is_new = {
            let observer = poller.observer();
            pollers.insert(observer, mask).is_none()
        };
        if is_new {
            let mut pollees = poller.inner.pollees.lock();
            pollees.push(Arc::downgrade(&self.inner));

            self.inner.num_pollers.fetch_add(1, Ordering::Release);
        }
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
            .for_each(|(poller, mask)| poller.on_events(self.inner.id.get(), events & *mask));
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

    /// Reset the pollee's state.
    ///
    /// Reset means removing all events on the pollee.
    pub fn reset_events(&self) {
        self.inner
            .events
            .fetch_and(!Events::all().bits(), Ordering::Release);
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
    pub fn register_observer(&self, observer: Arc<dyn Observer>, mask: Events) {
        let observer: KeyableArc<dyn Observer> = observer.into();
        let mask = mask | Events::ALWAYS_POLL;

        let mut pollers = self.inner.pollers.lock();
        let is_new = pollers.insert(observer, mask).is_none();
        if is_new {
            self.inner.num_pollers.fetch_add(1, Ordering::Release);
        }
    }

    /// Unregister an event observer.
    ///
    /// If such an observer is found, then the registered observer will be
    /// removed from the pollee and returned as the return value. Otherwise,
    /// a `None` will be returned.
    pub fn unregister_observer(&self, observer: &Arc<dyn Observer>) -> Option<Arc<dyn Observer>> {
        // Safety. This is safe since KeyableArc<T> has exactly the same memory representation
        // as Arc<T>.
        let observer: &KeyableArc<dyn Observer> = unsafe { core::mem::transmute(observer) };

        let mut pollers = self.inner.pollers.lock();
        let observer = pollers
            .remove_entry(observer)
            .map(|(observer, _mask)| observer.into());
        if observer.is_some() {
            self.inner.num_pollers.fetch_sub(1, Ordering::Release);
        }
        observer
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
    /// Constructs a new `Poller`.
    pub fn new() -> Self {
        let inner = PollerInner {
            event_counter: EventCounter::new(),
            pollees: Mutex::new(Vec::with_capacity(1)),
        };
        Self {
            inner: KeyableArc::new(inner),
        }
    }

    /// Wait until there are any interesting events happen since last `wait`.
    pub async fn wait(&self) -> Result<u64> {
        self.inner.event_counter.read().await
    }

    /// Wait until there are any interesting events happen since last `wait`, or reach timeout.
    pub async fn wait_timeout<T: BorrowMut<Duration>>(
        &self,
        timeout: Option<&mut T>,
    ) -> Result<()> {
        self.inner
            .event_counter
            .read_timeout(timeout)
            .await
            .map(|_| ())
    }

    pub fn observer(&self) -> KeyableArc<dyn Observer> {
        self.inner.clone() as KeyableArc<dyn Observer>
    }
}

impl Observer for PollerInner {
    fn on_events(&self, _pollee_id: u64, _events: Events) {
        self.event_counter.write();
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
                let mut pollers = pollee.pollers.lock();
                let res = pollers.remove(&self_observer);
                assert!(res.is_some());
                drop(pollers);

                pollee.num_pollers.fetch_sub(1, Ordering::Relaxed);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poll() {
        let mut poller = Poller::new();
        let pollee = Pollee::new(Events::empty());
        assert!(pollee.poll(Events::IN, Some(&mut poller)) == Events::empty());
    }

    #[test]
    fn subscribe() {
        use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

        struct Counter {
            pollee_id: u64,
            count: AtomicU64,
        }

        impl Counter {
            pub fn new(pollee_id: u64) -> Self {
                Self {
                    pollee_id,
                    count: AtomicU64::new(0),
                }
            }

            pub fn count(&self) -> u64 {
                self.count.load(Relaxed)
            }
        }

        impl Observer for Counter {
            fn on_events(&self, pollee_id: u64, _events: Events) {
                assert!(self.pollee_id == pollee_id);
                self.count.fetch_add(1, Relaxed);
            }
        }

        let pollee = Pollee::new(Events::empty());
        let counter = Arc::new(Counter::new(pollee.id().get()));
        pollee.register_observer(counter.clone(), Events::IN);
        let expected_count = 10;
        (0..expected_count).for_each(|_| pollee.add_events(Events::IN));
        pollee.unregister_observer(&(counter.clone() as _));
        assert!(counter.count() == expected_count);
    }
}
