use super::*;

use super::user_space_vm::USER_SPACE_VM_MANAGER;
use bitvec::vec::BitVec;
use std::sync::SgxRwLock;
use vm_epc::EPC;

// In SGX v2, there is no upper limit for the size of EPC. If the user configure 1 TB memory,
// and we only use one bit to track if the page is committed, that's 1 TB / 4 kB / 8 bit = 32 MB of memory.
// And the memory footprint will keep the same size during the whole libOS life cycle.
// In order to track the commit status of a huge number of pages, use two level tracking.
// In the first level, global level, we use 1 GB as the unit size for a page chunk.
// In the second level, we just use the page size as the unit size, and use one bit to represent if the page is committed.
// For example, if the user configure 64 TB memory, when a page is committed, the second level tracker will mark the correponding bit as 1.
// And when a whole 1 GB of pages are fully committed, the global level tracker will mark the page chunk as fully committed.
// And the corresponding tracker can be freed. In this way, we can use just several bytes to represent the commit status of 1 GB memory.
// In a worse case, let's say there are several discrete global page chunks which are not not fully committed at the same time.
// And each of them will take some space in the memory. Within a memory-intensive case, we can
// commit the page by hand and make the global page chunk fully committed and free the page tracker.

// There are mainly three types of data structure to track the page status, from the top to the bottom:
// 1. PageChunkManager - Create for the whole user space. This sructure is used to manage the global paging status.
// 2. GlobalPageChunk - Denotes a chunk of pages. The actual unit of the PageChunkManager. It holds the paging status of a memory range. Stored only
// in the PageChunkManager. A newly created VMA should ask the corresponding GlobalPageChunk for the paging status. When all the pages recoreded by
// GlobalPageChunk are all committed, it will mark itself as "fully committed" and free the inner structure tracking the paging status. All the GlobalPageChunk
// records the VM ranges with the SAME size.
// 3. PageTracker - The real tracker of the paging status. Under the hood, it is a bitvec that tracks every page with a bit. There are mainly two types
// PageTracker:
//      * GlobalTracker - Used by GlobalPageChunk to track the paging status. All records the VM range with the same size.
//      * VMATracker - Used by VMA to track its paging status. Records different range size according to the VMA.
// Since the VM operations are mostly performed by VMA, the VMA tracker will update itself accordingly. And also update the corresponding GlobalTracker.

lazy_static! {
    pub static ref USER_SPACE_PAGE_CHUNK_MANAGER: SgxRwLock<PageChunkManager> =
        SgxRwLock::new(PageChunkManager::new(USER_SPACE_VM_MANAGER.range()));
}

const GB: usize = 1 << 30;
const TB: usize = 1 << 40;
const MB: usize = 1 << 20;
const KB: usize = 1 << 10;

const PageChunkUnit: usize = 1 * MB;
const PageChunkPageNum: usize = PageChunkUnit / PAGE_SIZE;

pub struct PageChunkManager {
    // The total range that the manager manages.
    range: VMRange,
    // The page chunks
    inner: HashMap<usize, GlobalPageChunk>, // K: Page chunk start address, V: Global page chunk
}

impl PageChunkManager {
    fn new(range: &VMRange) -> Self {
        Self {
            range: range.clone(),
            inner: HashMap::new(),
        }
    }

    fn page_chunk_unit() -> usize {
        PageChunkUnit
    }
}

#[derive(Debug)]
// A chunk of pages. Memory space is precious. Don't put anything unnecessary.
struct GlobalPageChunk {
    fully_committed: bool,
    tracker: Option<Arc<SgxRwLock<PageTracker>>>, // if this page chunk is fully committed, the tracker will be set to None.
}

impl GlobalPageChunk {
    fn new(tracker: PageTracker) -> Self {
        Self {
            fully_committed: false,
            tracker: Some(Arc::new(SgxRwLock::new(tracker))),
        }
    }
}

#[derive(PartialEq, Clone, Debug)]
enum TrackerType {
    GlobalTracker, // PageChunkUnit size for global management to track the global paging status
    VMATracker,    // various size for different vma to track its own paging status
    GapTracker,    // Tracking for range which shouldn't be touched by users
}

// 1GB page chunk or VMA track its pages
#[derive(Clone)]
pub struct PageTracker {
    type_: TrackerType,
    range: VMRange,
    inner: BitVec,
    fully_committed: bool,
}

impl Debug for PageTracker {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("PageTracker")
            .field("type", &self.type_)
            .field("range", &self.range)
            .field("inner", &self.inner)
            .field("fully committed", &self.fully_committed)
            .finish()
    }
}

impl PageTracker {
    // Create a new page tracker for GlobalPageChunk.
    // When a new global tracker is needed, none of the pages are committed.
    fn new_global_tracker(start_addr: usize) -> Result<Self> {
        let range = VMRange::new_with_size(start_addr, PageChunkUnit)?;
        debug_assert!({
            // Global tracker must be created for user region memory
            use super::vm_epc::EPC;
            let epc_type = EPC::new(&range);
            matches!(epc_type, EPC::UserRegionMem(_))
        });

        let inner = bitvec![0; PageChunkPageNum];
        Ok(Self {
            type_: TrackerType::GlobalTracker,
            range,
            inner,
            fully_committed: false,
        })
    }

    pub fn new_vma_tracker(vm_range: &VMRange, epc_type: &EPC) -> Result<Self> {
        let page_num = vm_range.size() / PAGE_SIZE;
        let new_vma_tracker = match epc_type {
            EPC::UserRegionMem(_) => {
                let mut new_vma_tracker = Self {
                    type_: TrackerType::VMATracker,
                    range: vm_range.clone(),
                    inner: bitvec![0; page_num],
                    fully_committed: false,
                };

                // Skip sentry
                if page_num != 0 {
                    new_vma_tracker.update_committed_pages_from_global()?;
                }
                new_vma_tracker
            }
            EPC::ReservedMem(_) => {
                // For reserved memory, there is no need to udpate global page tracker.
                // And there is no GLobalPageChunk for reserved memory.
                Self {
                    type_: TrackerType::VMATracker,
                    range: vm_range.clone(),
                    inner: bitvec![1; page_num],
                    fully_committed: true,
                }
            }
            _ => unreachable!(),
        };

        Ok(new_vma_tracker)
    }

    pub fn new_gap_tracker(gap_range: &VMRange) -> Result<Self> {
        let page_num = gap_range.size() / PAGE_SIZE;
        let new_gap_tracker = Self {
            type_: TrackerType::GapTracker,
            range: gap_range.clone(),
            inner: bitvec![0; page_num],
            fully_committed: false,
        };

        Ok(new_gap_tracker)
    }

    pub fn range(&self) -> &VMRange {
        &self.range
    }

    pub fn is_fully_committed(&self) -> bool {
        self.fully_committed
    }

    pub fn is_reserved_only(&self) -> bool {
        !self.fully_committed && self.inner.not_any()
    }

    pub fn is_partially_committed(&self) -> bool {
        !self.fully_committed && self.inner.any()
    }

    // Get all committed or uncommitted ranges of consecutive page.
    // If committed is true, get all committed ranges
    // If committed is false, get all uncommitted ranges
    pub fn get_ranges(&self, committed: bool) -> Vec<VMRange> {
        if self.is_fully_committed() {
            if committed {
                return vec![self.range.clone()];
            } else {
                return Vec::new();
            }
        }
        if self.is_reserved_only() {
            if committed {
                return Vec::new();
            } else {
                return vec![self.range.clone()];
            }
        }

        let tracker_start_addr = self.range.start();
        let mut ret = Vec::new();
        let mut start = None;
        let mut end = None;
        let mut i = 0;
        while i < self.inner.len() {
            if self.inner[i] == committed {
                match (start, end) {
                    // Meet committed page for the first time. Update both the start and end marker.
                    (None, None) => {
                        start = Some(i);
                        end = Some(i);
                        // Reach the end of the tracker. Only one page
                        if i == self.inner.len() - 1 {
                            let committed_range = VMRange::new_with_size(
                                tracker_start_addr + i * PAGE_SIZE,
                                PAGE_SIZE,
                            )
                            .unwrap();
                            ret.push(committed_range);
                        }
                    }
                    // Previous pages are committed. Update the end marker.
                    (Some(s), Some(e)) => {
                        end = Some(i);
                        // Reach the end of the tracker.
                        if i == self.inner.len() - 1 {
                            let committed_range = VMRange::new_with_size(
                                tracker_start_addr + s * PAGE_SIZE,
                                PAGE_SIZE * (i - s + 1),
                            )
                            .unwrap();
                            ret.push(committed_range);
                        }
                    }
                    _ => unreachable!(),
                }
                i += 1;
            } else {
                match (start, end) {
                    (None, None) => {
                        // No committed pages.
                    }
                    (Some(s), Some(e)) => {
                        // Meet the first uncommitted pages after recording all the previous committed pages.
                        let committed_range = VMRange::new_with_size(
                            tracker_start_addr + s * PAGE_SIZE,
                            PAGE_SIZE * (e - s + 1),
                        )
                        .unwrap();
                        ret.push(committed_range);
                        // Reset markers
                        start = None;
                        end = None;
                    }
                    _ => {
                        unreachable!()
                    }
                }
                i += 1;
            }
        }

        let total_size = ret.iter().fold(0, |a, b| a + b.size());
        if committed {
            trace!("get committed ranges = {:?}", ret);
            debug_assert!(total_size == self.inner.count_ones() * PAGE_SIZE);
        } else {
            trace!("get uncommitted ranges = {:?}", ret);
            debug_assert!(total_size == self.inner.count_zeros() * PAGE_SIZE);
        }

        ret
    }

    // Commit memory for the whole current VMA (VMATracker)
    pub fn commit_current_vma(&mut self, need_update_global: bool) -> Result<()> {
        debug_assert!(self.type_ == TrackerType::VMATracker);

        if self.is_fully_committed() {
            return Ok(());
        }

        // Commit EPC
        if self.is_reserved_only() {
            vm_epc::commit_epc_for_user_space(self.range().start(), self.range().size()).unwrap();
        } else {
            debug_assert!(self.is_partially_committed());
            let uncommitted_ranges = self.get_ranges(false);
            for range in uncommitted_ranges {
                vm_epc::commit_epc_for_user_space(range.start(), range.size()).unwrap();
            }
        }

        // Update the tracker
        self.inner.fill(true);
        self.fully_committed = true;

        if need_update_global {
            self.update_pages_for_global_tracker(self.range().start(), self.range().size());
        }

        Ok(())
    }

    // Commit memory of a specific range for the current VMA (VMATracker). The range should be verified by caller.
    pub fn commit_range_for_current_vma(&mut self, range: &VMRange) -> Result<()> {
        debug_assert!(self.type_ == TrackerType::VMATracker);

        vm_epc::commit_epc_for_user_space(range.start(), range.size())?;

        self.commit_pages_internal(range.start(), range.size());
        self.update_pages_for_global_tracker(range.start(), range.size());

        Ok(())
    }

    // VMA tracker update page commit status from global tracker
    fn update_committed_pages_from_global(&mut self) -> Result<()> {
        debug_assert!(self.type_ == TrackerType::VMATracker);
        let mut vma_tracker = self;
        let mut page_chunk_addr = get_page_chunk_start_addr(vma_tracker.range().start());

        let range_end = vma_tracker.range().end();
        while (page_chunk_addr < range_end) {
            let manager = USER_SPACE_PAGE_CHUNK_MANAGER.read().unwrap();
            if let Some(page_chunk) = manager.inner.get(&page_chunk_addr) {
                if page_chunk.fully_committed {
                    // global page chunk fully committed. commit pages for vma page chunk
                    trace!("update_committed_pages_from_global 1");
                    vma_tracker.commit_pages_internal(page_chunk_addr, PageChunkUnit);
                } else {
                    debug_assert!(page_chunk.tracker.is_some());
                    trace!("update_committed_pages 2");
                    let global_tracker = page_chunk.tracker.as_ref().unwrap().read().unwrap();
                    global_tracker.update_committed_pages_for_vma_tracker(vma_tracker);
                }
                drop(manager);
            } else {
                // Not tracking this page chunk. Release read lock and acquire write lock for an update.
                drop(manager);
                // This page chunk is not tracked by global tracker. Thus none of the pages are committed.
                let page_chunk = {
                    let global_page_tracker = PageTracker::new_global_tracker(page_chunk_addr)?;

                    trace!(
                        "update_committed_pages 3. track new page chunk = {:?}",
                        global_page_tracker.range()
                    );
                    GlobalPageChunk::new(global_page_tracker)
                };

                // There could be data race here. But it's fine, because the ultimate state is the same.
                USER_SPACE_PAGE_CHUNK_MANAGER
                    .write()
                    .unwrap()
                    .inner
                    .insert(page_chunk_addr, page_chunk);
            }

            // Update page chunk addr for next loop
            page_chunk_addr = page_chunk_addr + PageChunkUnit;
        }

        Ok(())
    }

    // GlobalTracker helps to update VMATracker based on the paging status of itself.
    fn update_committed_pages_for_vma_tracker(&self, vma_tracker: &mut PageTracker) {
        debug_assert!(self.type_ == TrackerType::GlobalTracker);
        debug_assert!(vma_tracker.type_ == TrackerType::VMATracker);

        let global_tracker = self;

        if let Some(intersection_range) = global_tracker.range().intersect(vma_tracker.range()) {
            let vma_tracker_page_id =
                (intersection_range.start() - vma_tracker.range().start()) / PAGE_SIZE;
            let global_tracker_page_id =
                (intersection_range.start() - global_tracker.range().start()) / PAGE_SIZE;
            let page_num = intersection_range.size() / PAGE_SIZE;

            vma_tracker.inner[vma_tracker_page_id..vma_tracker_page_id + page_num]
                .copy_from_bitslice(
                    &global_tracker.inner
                        [global_tracker_page_id..global_tracker_page_id + page_num],
                );
            if vma_tracker.inner.all() {
                vma_tracker.fully_committed = true;
            }
        } else {
            // No intersection range, why calling this? Wierd.
            unreachable!();
        }
    }

    // VMAtracker helps to update global tracker based on the paging status of itself.
    fn update_pages_for_global_tracker(&self, commit_start_addr: usize, commit_size: usize) {
        debug_assert!(self.type_ == TrackerType::VMATracker);

        let commit_end_addr = commit_start_addr + commit_size;
        let mut page_chunk_start_addr = get_page_chunk_start_addr(commit_start_addr);
        while (page_chunk_start_addr < commit_end_addr) {
            let is_global_tracker_fully_committed = {
                // Find the correponding page chunk
                let manager = USER_SPACE_PAGE_CHUNK_MANAGER.read().unwrap();
                let page_chunk = manager
                    .inner
                    .get(&page_chunk_start_addr)
                    .expect("this page chunk must exist");

                // Update the global page tracker
                if let Some(global_page_tracker) = &page_chunk.tracker {
                    let mut global_tracker = global_page_tracker.write().unwrap();
                    global_tracker.commit_pages_internal(commit_start_addr, commit_size);
                    trace!("update_global_tracker 1");
                    global_tracker.fully_committed
                } else {
                    // page_tracker is none, the page chunk is fully committed. Go to next chunk.
                    debug_assert!(page_chunk.fully_committed);
                    trace!("update_global_tracker 2");
                    page_chunk_start_addr = page_chunk_start_addr + PageChunkUnit;
                    continue;
                }
            };

            // Free the global page tracker if fully committed
            if is_global_tracker_fully_committed {
                // Update the global page chunk manager. Need to acquire the write lock this time. There can be data race because the lock
                // could be dropped for a while before acquire again. But its fine, because the ultimate state is the same.
                let mut manager = USER_SPACE_PAGE_CHUNK_MANAGER.write().unwrap();
                if let Some(mut page_chunk) = manager.inner.get_mut(&page_chunk_start_addr) {
                    trace!("update_global_tracker 3");
                    page_chunk.fully_committed = true;
                    page_chunk.tracker = None;
                } else {
                    warn!(
                        "the global page chunk with start addr: 0x{:x} has been freed already",
                        page_chunk_start_addr
                    );
                    unreachable!();
                }
            }

            page_chunk_start_addr = page_chunk_start_addr + PageChunkUnit;
        }
    }

    // Commit pages for page tracker itself. This is a common method for both VMATracker and GlobalTracker.
    fn commit_pages_internal(&mut self, start_addr: usize, size: usize) {
        debug_assert!(self.type_ != TrackerType::GapTracker);
        if self.fully_committed {
            return;
        }

        if let Some(intersection_range) = {
            let range = VMRange::new_with_size(start_addr, size).unwrap();
            self.range.intersect(&range)
        } {
            warn!("commit for page tracker: {:?}", self);
            trace!("commit_pages intersection range = {:?}", intersection_range);
            let page_start_id = (intersection_range.start() - self.range().start()) / PAGE_SIZE;
            let page_num = intersection_range.size() / PAGE_SIZE;
            self.inner[page_start_id..page_start_id + page_num]
                .iter_mut()
                .for_each(|mut bit| *bit = true);
            if self.inner.all() {
                self.fully_committed = true;
            }
        } else {
            // No intersect range, wierd
            unreachable!();
        }
    }
}

#[inline(always)]
fn get_page_chunk_start_addr(addr: usize) -> usize {
    align_down(addr, PageChunkUnit)
}
