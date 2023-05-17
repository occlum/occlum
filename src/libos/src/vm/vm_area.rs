use std::ops::{Deref, DerefMut};

use super::vm_perms::VMPerms;
use super::vm_range::VMRange;
use super::vm_util::FileBacked;
use super::*;

use intrusive_collections::rbtree::{Link, RBTree};
use intrusive_collections::{intrusive_adapter, KeyAdapter};

#[derive(Clone, Debug, Default)]
pub struct VMArea {
    range: VMRange,
    perms: VMPerms,
    file_backed: Option<FileBacked>,
    pid: pid_t,
}

impl VMArea {
    pub fn new(
        range: VMRange,
        perms: VMPerms,
        file_backed: Option<FileBacked>,
        pid: pid_t,
    ) -> Self {
        Self {
            range,
            perms,
            file_backed,
            pid,
        }
    }

    /// Create a new VMArea object that inherits the write-back file (if any), but has
    /// a new range and permissions.
    pub fn inherits_file_from(
        vma: &VMArea,
        new_range: VMRange,
        new_perms: VMPerms,
        pid: pid_t,
    ) -> Self {
        let new_backed_file = vma.file_backed.as_ref().map(|file| {
            let mut new_file = file.clone();
            let file_offset = file.offset();

            let new_file_offset = if vma.start() < new_range.start() {
                let vma_offset = new_range.start() - vma.start();
                file_offset + vma_offset
            } else {
                let vma_offset = vma.start() - new_range.start();
                debug_assert!(file_offset >= vma_offset);
                file_offset - vma_offset
            };

            new_file.set_offset(new_file_offset);

            new_file
        });

        Self::new(new_range, new_perms, new_backed_file, pid)
    }

    pub fn perms(&self) -> VMPerms {
        self.perms
    }

    pub fn range(&self) -> &VMRange {
        &self.range
    }

    pub fn pid(&self) -> pid_t {
        self.pid
    }

    pub fn init_file(&self) -> Option<(&FileRef, usize)> {
        if let Some(file) = &self.file_backed {
            Some(file.init_file())
        } else {
            None
        }
    }

    pub fn writeback_file(&self) -> Option<(&FileRef, usize)> {
        if let Some(file) = &self.file_backed {
            file.writeback_file()
        } else {
            None
        }
    }

    pub fn set_perms(&mut self, new_perms: VMPerms) {
        self.perms = new_perms;
    }

    pub fn subtract(&self, other: &VMRange) -> Vec<VMArea> {
        self.deref()
            .subtract(other)
            .into_iter()
            .map(|range| Self::inherits_file_from(self, range, self.perms(), self.pid()))
            .collect()
    }

    // Returns an non-empty intersection if where is any
    pub fn intersect(&self, other: &VMRange) -> Option<VMArea> {
        let new_range = {
            let new_range = self.range().intersect(other);
            if new_range.is_none() {
                return None;
            }
            new_range.unwrap()
        };
        let new_vma = VMArea::inherits_file_from(self, new_range, self.perms(), self.pid());
        Some(new_vma)
    }

    pub fn resize(&mut self, new_size: usize) {
        self.range.resize(new_size)
    }

    pub fn set_start(&mut self, new_start: usize) {
        let old_start = self.start();
        self.range.set_start(new_start);

        if let Some(file) = self.file_backed.as_mut() {
            if !file.need_write_back() {
                return;
            }
            // If the updates to the VMA needs to write back to a file, then the
            // file offset must be adjusted according to the new start address.
            let offset = file.offset();
            if old_start < new_start {
                file.set_offset(offset + (new_start - old_start));
            } else {
                // The caller must guarantee that the new start makes sense
                debug_assert!(offset >= old_start - new_start);
                file.set_offset(offset - (old_start - new_start));
            }
        }
    }

    pub fn is_the_same_to(&self, other: &VMArea) -> bool {
        if self.pid() != other.pid() {
            return false;
        }

        if self.range() != other.range() {
            return false;
        }

        if self.perms() != other.perms() {
            return false;
        }

        let self_writeback_file = self.writeback_file();
        let other_writeback_file = other.writeback_file();
        match (self_writeback_file, other_writeback_file) {
            (None, None) => return true,
            (Some(_), None) => return false,
            (None, Some(_)) => return false,
            (Some((self_file, self_offset)), Some((other_file, other_offset))) => {
                self_file == other_file && self_offset == other_offset
            }
        }
    }

    pub fn set_end(&mut self, new_end: usize) {
        self.range.set_end(new_end);
    }

    pub fn can_merge_vmas(left: &VMArea, right: &VMArea) -> bool {
        debug_assert!(left.end() <= right.start());

        // Both of the two VMAs must not be sentry (whose size == 0)
        if left.size() == 0 || right.size() == 0 {
            return false;
        }
        // The two VMAs must be owned by the same process
        if left.pid() != right.pid() {
            return false;
        }
        // The two VMAs must border with each other
        if left.end() != right.start() {
            return false;
        }
        // The two VMAs must have the same memory permissions
        if left.perms() != right.perms() {
            return false;
        }

        // If the two VMAs have write-back files, the files must be the same and
        // the two file regions must be continuous.
        let left_writeback_file = left.writeback_file();
        let right_writeback_file = right.writeback_file();
        match (left_writeback_file, right_writeback_file) {
            (None, None) => true,
            (Some(_), None) => false,
            (None, Some(_)) => false,
            (Some((left_file, left_offset)), Some((right_file, right_offset))) => {
                left_file == right_file
                    && right_offset > left_offset
                    && right_offset - left_offset == left.size()
            }
        }
    }

    /// Flush a file-backed VMA to its file. This has no effect on anonymous VMA.
    pub async fn flush_backed_file(&self) {
        Self::flush_backed_file_with_cond(self, |_| true).await
    }

    /// Same as `flush_backed_file()`, except that an extra condition on the file needs to satisfy.
    pub async fn flush_backed_file_with_cond<F: Fn(&FileRef) -> bool>(&self, cond_fn: F) {
        let (file, file_offset) = match self.writeback_file() {
            None => return,
            Some((file_and_offset)) => file_and_offset,
        };
        let file_handle = file.as_async_file_handle().unwrap();
        let file_writable = file_handle.access_mode().writable();
        if !file_writable {
            return;
        }
        if !cond_fn(file) {
            return;
        }
        file_handle
            .dentry()
            .inode()
            .write_at(file_offset, unsafe { self.as_slice() })
            .await;
    }
}

impl Deref for VMArea {
    type Target = VMRange;

    fn deref(&self) -> &Self::Target {
        &self.range
    }
}

#[derive(Clone)]
pub struct VMAObj {
    link: Link,
    vma: VMArea,
}

impl fmt::Debug for VMAObj {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.vma)
    }
}

// key adapter for RBTree which is sorted by the start of vma ranges
intrusive_adapter!(pub VMAAdapter = Box<VMAObj>: VMAObj { link : Link });
impl<'a> KeyAdapter<'a> for VMAAdapter {
    type Key = usize;
    fn get_key(&self, vma_obj: &'a VMAObj) -> usize {
        vma_obj.vma.range().start()
    }
}

impl VMAObj {
    pub fn new_vma_obj(vma: VMArea) -> Box<Self> {
        Box::new(Self {
            link: Link::new(),
            vma,
        })
    }

    pub fn vma(&self) -> &VMArea {
        &self.vma
    }
}
