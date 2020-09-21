/// A trait to represent any event.
pub trait Event: Copy + Clone + Send + Sync + 'static {}

/// A trait to filter events.
pub trait EventFilter<E: Event>: Send + Sync + 'static {
    fn filter(&self, event: &E) -> bool;
}
