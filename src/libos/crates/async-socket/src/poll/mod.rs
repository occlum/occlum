mod event_counter;

use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(not(feature = "sgx"))]
use std::sync::{Arc, Mutex, Weak};
#[cfg(feature = "sgx")]
use std::sync::{Arc, SgxMutex as Mutex, Weak};

use atomic::Atomic;

pub use self::event_counter::EventCounter;

bitflags::bitflags! {
    /// I/O Events that can be polled.
    #[rustfmt::skip]
    pub struct Events: u32 {
        /// = POLLIN
        const IN    = 0x0001;
        /// = POLLPRI
        const PRI   = 0x0002;
        /// = POLLOUT
        const OUT   = 0x0004;
        /// = POLLERR
        const ERR   = 0x0008;
        /// = POLLHUP
        const HUP   = 0x0010;
        /// = POLLNVAL
        const NVAL  = 0x0020;
        /// = POLLRDHUP
        const RDHUP = 0x2000;
        /// Events that are always polled even without specifying them.
        const ALWAYS_POLL = Self::ERR.bits | Self::HUP.bits;
    }
}

/// A pollee has events as its state, which can be polled by a poller.
pub struct Pollee {
    inner: Arc<Pollee_>,
}

struct Pollee_ {
    id: u64,
    events: Atomic<Events>,
    pollers: Mutex<HashMap<WeakPoller_, Events>>,
}

// A wrapper to satisfy the traits required for a key type of HashMap/HashSet.
struct WeakPollee_(Weak<Pollee_>);

/// A poller polls the events on a pollee.
pub struct Poller {
    inner: Arc<Poller_>,
}

struct Poller_ {
    id: u64,
    pollees: Mutex<HashSet<WeakPollee_>>,
    event_counter: EventCounter,
}

// A wrapper to satisfy the traits required for a key type of HashMap/HashSet.
struct WeakPoller_(Weak<Poller_>);

// Implementation of Pollee & Pollee_

impl Pollee {
    pub fn new(init_events: Events) -> Self {
        let inner = Arc::new(Pollee_::new(init_events));
        Self { inner }
    }

    pub fn poll_by(&self, mask: Events, poller: Option<&mut Poller>) -> Events {
        let mask = mask | Events::ALWAYS_POLL;

        // Attempt to get interesting events without locking. It is ok to return false positives.
        let revents = self.events() & mask;
        if !revents.is_empty() || poller.is_none() {
            return revents;
        }
        let poller = poller.unwrap();

        // Lock note. Always acquire the lock of poller, then that of pollee.
        let mut pollees = poller.inner.pollees.lock().unwrap();
        let mut pollers = self.inner.pollers.lock().unwrap();

        // Try again after acquiring the lock
        let revents = self.events() & mask;
        if !revents.is_empty() {
            return revents;
        }

        // Make connection between the poller and the pollee
        let weak_poller = Arc::downgrade(&poller.inner).into();
        let weak_pollee = Arc::downgrade(&self.inner).into();
        pollers.insert(weak_poller, mask);
        pollees.get_or_insert(weak_pollee);
        Events::empty()
    }

    pub fn add(&self, events: Events) {
        let pollers = self.inner.pollers.lock().unwrap();
        let new_events = self.events() | events;
        self.inner.events.store(new_events, Ordering::Relaxed);

        pollers
            .iter()
            .filter(|(_, mask)| mask.intersects(events))
            .for_each(|(weak_poller, _)| {
                if let Some(poller) = weak_poller.upgrade() {
                    poller.wake();
                }
            });
    }

    pub fn remove(&self, events: Events) {
        let pollers = self.inner.pollers.lock().unwrap();
        let new_events = self.events() & !events;
        self.inner.events.store(new_events, Ordering::Relaxed);
    }

    fn events(&self) -> Events {
        self.inner.events.load(Ordering::Relaxed)
    }
}

impl Pollee_ {
    pub fn new(init_events: Events) -> Self {
        let id = {
            static NEXT_ID: AtomicU64 = AtomicU64::new(0);
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        };
        let events = Atomic::new(init_events);
        let pollers = Mutex::new(HashMap::new());
        Self {
            id,
            events,
            pollers,
        }
    }
}

// Implementation of Poller and Poller_

impl Poller {
    pub fn new() -> Self {
        let inner = Arc::new(Poller_::new());
        Self { inner }
    }

    pub async fn wait(&self) {
        self.inner.wait().await
    }
}

impl Drop for Poller {
    fn drop(&mut self) {
        // Detach this poller from all its pollee to prevent memory leakage.
        let weak_self = Arc::downgrade(&self.inner).into();
        let mut pollees = self.inner.pollees.lock().unwrap();
        for weak_pollee in pollees.drain() {
            if let Some(pollee) = weak_pollee.upgrade() {
                let mut pollers = pollee.pollers.lock().unwrap();
                pollers.remove(&weak_self);
            }
        }
    }
}

impl Poller_ {
    pub fn new() -> Self {
        let id = {
            static NEXT_ID: AtomicU64 = AtomicU64::new(0);
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        };
        let pollees = Mutex::new(HashSet::new());
        let event_counter = EventCounter::new(0);
        Self {
            id,
            pollees,
            event_counter,
        }
    }

    pub async fn wait(&self) {
        self.event_counter.read().await;
    }

    pub fn wake(&self) {
        self.event_counter.write();
    }
}

// Implementation for WeakPollee_

impl WeakPollee_ {
    pub fn upgrade(&self) -> Option<Arc<Pollee_>> {
        self.0.upgrade()
    }
}

impl From<Weak<Pollee_>> for WeakPollee_ {
    fn from(weak: Weak<Pollee_>) -> Self {
        Self(weak)
    }
}

impl PartialEq for WeakPollee_ {
    fn eq(&self, other: &Self) -> bool {
        Weak::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for WeakPollee_ {}

impl Hash for WeakPollee_ {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.as_ptr().hash(state);
    }
}

// Implementation for WeakPoller_

impl WeakPoller_ {
    pub fn upgrade(&self) -> Option<Arc<Poller_>> {
        self.0.upgrade()
    }
}

impl From<Weak<Poller_>> for WeakPoller_ {
    fn from(weak: Weak<Poller_>) -> Self {
        Self(weak)
    }
}

impl PartialEq for WeakPoller_ {
    fn eq(&self, other: &Self) -> bool {
        Weak::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for WeakPoller_ {}

impl Hash for WeakPoller_ {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.as_ptr().hash(state);
    }
}
