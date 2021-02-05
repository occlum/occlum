pub mod lru_list;
pub mod object_id;

pub const fn align_up(val: usize, align: usize) -> usize {
    (val + align - 1) / align * align
}

pub const fn align_down(val: usize, align: usize) -> usize {
    val / align * align
}
