//! Cache state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CacheState {
    /// `Vacant` indicates cache has available space,
    /// the cache is ready to read or write.
    Vacant,
    /// `Full` indicates cache capacity is run out,
    /// the cache cannot write, can read.
    Full,
    /// `Flushing` indicates cache is being flushed to disk,
    /// the cache cannot write, can read.
    Flushing,
    /// `Clearing` indicates cache is being cleared out,
    /// the cache cannot write or read.
    Clearing,
}

impl CacheState {
    /// Check if state transition is legal
    pub fn examine_state_transition(old_state: CacheState, new_state: CacheState) {
        match old_state {
            CacheState::Vacant => debug_assert!(new_state == CacheState::Full),
            CacheState::Full => debug_assert!(new_state == CacheState::Flushing),
            CacheState::Flushing => debug_assert!(new_state == CacheState::Clearing),
            CacheState::Clearing => debug_assert!(new_state == CacheState::Vacant),
        }
    }
}
