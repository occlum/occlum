use std::any::Any;
use std::sync::Weak;

use super::Event;
use crate::prelude::*;

/// An obsever receives events from the notifiers to which it has registered.
pub trait Observer<E: Event>: Send + Sync {
    /// The callback that will be executed when some interesting events are
    /// delivered by a notifier to this observer.
    ///
    /// Note that it is important to keep this method simple, short, and
    /// non-blocking. This is because the caller of this function, which is most
    /// likely to be `Notifier::broadcast`, may have acquired the locks of some
    /// resources. In general, these locks may coincide with the ones required
    /// by a specific implementation of `Observer::on_event`. Thus, to avoid
    /// the odds of deadlocks, the `on_event` method should be written short
    /// and sweet.
    fn on_event(&self, event: &E, metadata: &Option<Weak<dyn Any + Send + Sync>>) -> ();
}
