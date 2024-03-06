pub fn align_down(addr: usize, align: usize) -> usize {
    debug_assert!(align.is_power_of_two());
    addr & !(align - 1)
}

pub fn align_up(addr: usize, align: usize) -> usize {
    debug_assert!(align.is_power_of_two());
    align_down(addr + (align - 1), align)
}
