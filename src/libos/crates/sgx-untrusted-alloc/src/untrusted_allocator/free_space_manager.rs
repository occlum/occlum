// Implements free space management for memory.
// Currently only use simple vector as the base structure.
//
// Basically use address-ordered first fit to find free ranges.
use super::*;
use std::cmp::Ordering;

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

    pub fn find_free_range_internal(&mut self, size: usize, align: usize) -> Result<VMRange> {
        // Record the minimal free range that satisfies the constraints
        let mut result_free_range: Option<VMRange> = None;
        let mut result_idx: Option<usize> = None;
        let free_list = &self.free_manager;

        for (idx, free_range) in free_list.iter().enumerate() {
            let free_range = {
                if free_range.size() < size {
                    continue;
                }

                // Check alignment
                let start = align_up(free_range.start(), align);
                let end = start + size;
                if free_range.end() < end {
                    continue;
                }
                unsafe { VMRange::from_unchecked(start, end) }
            };

            result_free_range = Some(free_range);
            result_idx = Some(idx);
            break;
        }

        if result_free_range.is_none() {
            return_errno!(ENOMEM, "not enough memory");
        }

        let index = result_idx.unwrap();
        let result_free_range = result_free_range.unwrap();

        self.free_list_update_range(index, result_free_range);
        return Ok(result_free_range);
    }

    fn free_list_update_range(&mut self, index: usize, range: VMRange) {
        let free_list = &mut self.free_manager;
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
        let free_list = &mut self.free_manager;

        // If the free list is empty, insert the dirty range and it's done.
        if free_list.is_empty() {
            free_list.push(*dirty_range);
            return Ok(());
        }

        let dirty_range_start = dirty_range.start();
        let dirty_range_end = dirty_range.end();

        // If the dirty range is before the first free range or after the last free range
        let head_range = &mut free_list[0];
        match dirty_range_end.cmp(&head_range.start()) {
            Ordering::Equal => {
                head_range.set_start(dirty_range_start);
                return Ok(());
            }
            Ordering::Less => {
                free_list.insert(0, *dirty_range);
                return Ok(());
            }
            _ => (),
        }

        let tail_range = free_list.last_mut().unwrap();
        match dirty_range_start.cmp(&tail_range.end()) {
            Ordering::Equal => {
                tail_range.set_end(dirty_range_end);
                return Ok(());
            }
            Ordering::Greater => {
                free_list.push(*dirty_range);
                return Ok(());
            }
            _ => (),
        }

        // The dirty range must be between some two ranges.
        debug_assert!(free_list.len() >= 2);
        let mut idx = 0;

        while idx < free_list.len() - 1 {
            let left_range = free_list[idx];
            let right_range = free_list[idx + 1];

            if left_range.end() <= dirty_range_start && dirty_range_end <= right_range.start() {
                match (
                    dirty_range.is_contiguous_with(&left_range),
                    dirty_range.is_contiguous_with(&right_range),
                ) {
                    (true, true) => {
                        let left_range = &mut free_list[idx];
                        left_range.set_end(right_range.end());
                        free_list.remove(idx + 1);
                    }
                    (true, false) => {
                        let left_range = &mut free_list[idx];
                        left_range.set_end(dirty_range_end);
                    }
                    (false, true) => {
                        let right_range = &mut free_list[idx + 1];
                        right_range.set_start(dirty_range_start);
                    }
                    (false, false) => {
                        free_list.insert(idx + 1, *dirty_range);
                    }
                }
                break;
            }
            idx += 1;
        }
        return Ok(());
    }

    pub fn is_free_range(&self, request_range: &VMRange) -> bool {
        self.free_manager
            .iter()
            .any(|free_range| free_range.is_superset_of(request_range))
    }
}
