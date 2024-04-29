use std::time::Duration;

use super::HostEventFd;
use crate::prelude::*;

/// This trait abstracts over the synchronization mechanism to allow for different implementations that can
/// interact with the host's file descriptor based event notification mechanisms or other kinds of notification facilities.

pub trait Synchronizer {
    /// Creates and returns a new instance of a synchronization primitive.
    fn new() -> Self;

    /// Resets the synchronization primitive state.
    fn reset(&self);

    /// Waits for the synchronization event to occur until an optional `timeout` duration has elapsed.
    fn wait(&self, timeout: Option<&Duration>) -> Result<()>;

    /// Similar to `wait` but allows a mutable `timeout` parameter that can be adjusted to reflect the remaining
    /// time for the wait operation.
    fn wait_mut(&self, timeout: Option<&mut Duration>) -> Result<()>;

    /// Wakes one or more threads waiting on this synchronization primitive
    fn wake(&self);

    /// Returns a reference to the `host_eventfd`, an object tied to a file descriptor used for event notifications.
    fn host_eventfd(&self) -> &HostEventFd;

    /// Determines the condition under which a wake event should be triggered.
    fn wake_cond(&self) -> bool;
}
