//! Data region.
mod cache;
mod cleaning;
mod state;

pub(crate) use cache::{DataBlock, DataCache};
pub(crate) use cleaning::Cleaner;
pub(crate) use state::CacheState;
