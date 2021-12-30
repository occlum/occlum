// Implements free space management for memory.
// Currently only use simple vector as the base structure.
//
// Basically use address-ordered first fit to find free ranges.

use super::vm_util::VMMapAddr;
use super::*;

static INITIAL_SIZE: usize = 100;

#[derive(Debug, Default)]
pub struct VMFreeSpaceManager {
    free_manager: Vec<VMRange>, // Address-ordered first fit
}

impl VMFreeSpaceManager {
    pub fn new(initial_free_range: VMRange) -> Self {
        let mut free_manager = Vec::with_capacity(INITIAL_SIZE);
        free_manager.push(initial_free_range);

        VMFreeSpaceManager {
            free_manager: free_manager,
        }
    }

    pub fn free_size(&self) -> usize {
        self.free_manager
            .iter()
            .fold(0, |acc, free_range| acc + free_range.size())
    }

    // TODO: respect options.align when mmap
    pub fn find_free_range_internal(
        &mut self,
        size: usize,
        align: usize,
        addr: VMMapAddr,
    ) -> Result<VMRange> {
        // Record the minimal free range that satisfies the contraints
        let mut result_free_range: Option<VMRange> = None;
        let mut result_idx: Option<usize> = None;
        let mut free_list = &mut self.free_manager;

        trace!("find free range, free list = {:?}", free_list);

        for (idx, free_range) in free_list.iter().enumerate() {
            let mut free_range = {
                if free_range.size() < size {
                    continue;
                }
                free_range.clone()
            };

            match addr {
                // Want a minimal free_range
                VMMapAddr::Any => {}
                // Prefer to have free_range.start == addr
                VMMapAddr::Hint(addr) => {
                    if addr % align == 0
                        && free_range.contains(addr)
                        && free_range.end() - addr >= size
                    {
                        free_range.start = addr;
                        free_range.end = addr + size;
                        self.free_list_update_range(idx, free_range);
                        return Ok(free_range);
                    } else {
                        // Hint failure, record the result but keep iterating.
                        if result_free_range == None
                            || result_free_range.as_ref().unwrap().size() > free_range.size()
                        {
                            result_free_range = Some(free_range);
                            result_idx = Some(idx);
                        }
                        continue;
                    }
                }
                // Must have free_range.start == addr
                VMMapAddr::Need(addr) | VMMapAddr::Force(addr) => {
                    if free_range.start() > addr {
                        return_errno!(ENOMEM, "not enough memory for fixed mmap");
                    }
                    if !free_range.contains(addr) {
                        continue;
                    }
                    if free_range.end() - addr < size {
                        return_errno!(ENOMEM, "not enough memory for fixed mmap");
                    }
                    free_range.start = addr;
                    free_range.end = addr + size;
                }
            }

            result_free_range = Some(free_range);
            result_idx = Some(idx);
            break;
        }

        if result_free_range.is_none() {
            return_errno!(ENOMEM, "not enough memory");
        }

        let index = result_idx.unwrap();
        let result_free_range = {
            let free_range = result_free_range.unwrap();
            let start = align_up(free_range.start(), align);
            let end = start + size;
            VMRange { start, end }
        };

        self.free_list_update_range(index, result_free_range);
        trace!("after find free range, free list = {:?}", self.free_manager);
        return Ok(result_free_range);
    }

    fn free_list_update_range(&mut self, index: usize, range: VMRange) {
        let mut free_list = &mut self.free_manager;
        let ranges_after_subtraction = free_list[index].subtract(&range);
        debug_assert!(ranges_after_subtraction.len() <= 2);
        if ranges_after_subtraction.len() == 0 {
            free_list.remove(index);
            return;
        }
        free_list[index] = ranges_after_subtraction[0];
        if ranges_after_subtraction.len() == 2 {
            free_list.insert(index + 1, ranges_after_subtraction[1]);
        }
    }

    pub fn add_range_back_to_free_manager(&mut self, dirty_range: &VMRange) -> Result<()> {
        let mut free_list = &mut self.free_manager;
        free_list.push(*dirty_range);
        // Sort and merge small ranges
        free_list.sort_unstable_by(|range_a, range_b| range_a.start().cmp(&range_b.start()));
        let mut idx = 0;
        while (idx < free_list.len() - 1) {
            let right_range = free_list[idx + 1];
            let mut left_range = &mut free_list[idx];
            if left_range.end() == right_range.start() {
                left_range.set_end(right_range.end());
                free_list.remove(idx + 1);
                continue;
            }
            idx += 1;
        }
        trace!("after add range back free list = {:?}", free_list);
        return Ok(());
    }

    pub fn is_free_range(&self, request_range: &VMRange) -> bool {
        self.free_manager
            .iter()
            .any(|free_range| free_range.is_superset_of(request_range))
    }
}
