//! Utility.
pub mod cryption;
mod disk_array;
mod disk_range;
mod disk_view;
mod range_query_ctx;
pub mod serialize;

pub(crate) use disk_array::DiskArray;
pub(crate) use disk_range::{HbaRange, LbaRange};
pub(crate) use disk_view::DiskView;
pub(crate) use range_query_ctx::RangeQueryCtx;

pub type BitMap = bitvec::prelude::BitVec<u8, bitvec::prelude::Lsb0>;

pub(crate) const fn align_down(x: usize, align: usize) -> usize {
    debug_assert!(align >= 2 && align.is_power_of_two());
    (x / align) * align
}

pub(crate) const fn align_up(x: usize, align: usize) -> usize {
    debug_assert!(align >= 2 && align.is_power_of_two());
    ((x + align - 1) / align) * align
}
