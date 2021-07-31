use object_id::ObjectId;

use super::Events;

/// An observer for events on pollees.
///
/// In a sense, event observers are just a fancy form of callback functions.
/// An observer's `on_events` methods are supposed to be called when
/// some events that are interesting to the observer happen.
/// See `Pollee` for more information on how observers should be used.
pub trait Observer: Send + Sync + 'static {
    /// Notify the observer that some interesting events happen on the pollee.
    fn on_events(&self, pollee_id: u64, events: Events);
}
