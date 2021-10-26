use super::*;

// use super::vm_area::VMArea;
// use super::free_space_manager::VMFreeSpaceManager;
use super::vm_area::*;
use super::vm_perms::VMPerms;
use std::collections::BTreeSet;

use intrusive_collections::rbtree::{Link, RBTree};
use intrusive_collections::Bound;
use intrusive_collections::RBTreeLink;
use intrusive_collections::{intrusive_adapter, KeyAdapter};

#[derive(Clone, Debug)]
pub enum VMInitializer {
    DoNothing(),
    FillZeros(),
    CopyFrom {
        range: VMRange,
    },
    LoadFromFile {
        file: FileRef,
        offset: usize,
    },
    // For file-backed mremap which may move from old range to new range and read extra bytes from file
    CopyOldAndReadNew {
        old_range: VMRange,
        file: FileRef,
        offset: usize, // read file from this offset
    },
}

impl Default for VMInitializer {
    fn default() -> VMInitializer {
        VMInitializer::DoNothing()
    }
}

impl VMInitializer {
    pub fn init_slice(&self, buf: &mut [u8]) -> Result<()> {
        match self {
            VMInitializer::DoNothing() => {
                // Do nothing
            }
            VMInitializer::FillZeros() => {
                for b in buf {
                    *b = 0;
                }
            }
            VMInitializer::CopyFrom { range } => {
                let src_slice = unsafe { range.as_slice() };
                let copy_len = min(buf.len(), src_slice.len());
                buf[..copy_len].copy_from_slice(&src_slice[..copy_len]);
                for b in &mut buf[copy_len..] {
                    *b = 0;
                }
            }
            VMInitializer::LoadFromFile { file, offset } => {
                // TODO: make sure that read_at does not move file cursor
                let len = file
                    .read_at(*offset, buf)
                    .cause_err(|_| errno!(EACCES, "failed to init memory from file"))?;
                for b in &mut buf[len..] {
                    *b = 0;
                }
            }
            VMInitializer::CopyOldAndReadNew {
                old_range,
                file,
                offset,
            } => {
                // TODO: Handle old_range with non-readable subrange
                let src_slice = unsafe { old_range.as_slice() };
                let copy_len = src_slice.len();
                debug_assert!(copy_len <= buf.len());
                let read_len = buf.len() - copy_len;
                buf[..copy_len].copy_from_slice(&src_slice[..copy_len]);
                let len = file
                    .read_at(*offset, &mut buf[copy_len..])
                    .cause_err(|_| errno!(EACCES, "failed to init memory from file"))?;
                for b in &mut buf[(copy_len + len)..] {
                    *b = 0;
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum VMMapAddr {
    Any,          // Free to choose any address
    Hint(usize),  // Prefer the address, but can use other address
    Need(usize),  // Need to use the address, otherwise report error
    Force(usize), // Force using the address by munmap first
}

impl Default for VMMapAddr {
    fn default() -> VMMapAddr {
        VMMapAddr::Any
    }
}

#[derive(Builder, Debug)]
#[builder(pattern = "owned", build_fn(skip), no_std)]
pub struct VMMapOptions {
    size: usize,
    align: usize,
    perms: VMPerms,
    addr: VMMapAddr,
    initializer: VMInitializer,
    // The content of the VMA can be written back to a given file at a given offset
    writeback_file: Option<(FileRef, usize)>,
}

// VMMapOptionsBuilder is generated automatically, except the build function
impl VMMapOptionsBuilder {
    pub fn build(mut self) -> Result<VMMapOptions> {
        let size = {
            let size = self
                .size
                .ok_or_else(|| errno!(EINVAL, "invalid size for mmap"))?;
            if size == 0 {
                return_errno!(EINVAL, "invalid size for mmap");
            }
            align_up(size, PAGE_SIZE)
        };
        let align = {
            let align = self.align.unwrap_or(PAGE_SIZE);
            if align == 0 || !align.is_power_of_two() {
                return_errno!(EINVAL, "invalid size for mmap");
            }
            align
        };
        let perms = self
            .perms
            .ok_or_else(|| errno!(EINVAL, "perms must be given"))?;
        let addr = {
            let addr = self.addr.unwrap_or_default();
            match addr {
                // TODO: check addr + size overflow
                VMMapAddr::Any => VMMapAddr::Any,
                VMMapAddr::Hint(addr) => {
                    let addr = align_down(addr, PAGE_SIZE);
                    VMMapAddr::Hint(addr)
                }
                VMMapAddr::Need(addr_) | VMMapAddr::Force(addr_) => {
                    if addr_ % align != 0 {
                        return_errno!(EINVAL, "unaligned addr for fixed mmap");
                    }
                    addr
                }
            }
        };
        let initializer = match self.initializer.as_ref() {
            Some(initializer) => initializer.clone(),
            None => VMInitializer::default(),
        };
        let writeback_file = self.writeback_file.take().unwrap_or_default();
        Ok(VMMapOptions {
            size,
            align,
            perms,
            addr,
            initializer,
            writeback_file,
        })
    }
}

impl VMMapOptions {
    pub fn size(&self) -> &usize {
        &self.size
    }

    pub fn addr(&self) -> &VMMapAddr {
        &self.addr
    }

    pub fn perms(&self) -> &VMPerms {
        &self.perms
    }

    pub fn align(&self) -> &usize {
        &self.align
    }

    pub fn initializer(&self) -> &VMInitializer {
        &self.initializer
    }

    pub fn writeback_file(&self) -> &Option<(FileRef, usize)> {
        &self.writeback_file
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum VMRemapSizeType {
    Same,
    Shrinking,
    Growing,
}

impl VMRemapSizeType {
    pub fn new(old_size: &usize, new_size: &usize) -> Self {
        if new_size == old_size {
            VMRemapSizeType::Same
        } else if new_size < old_size {
            VMRemapSizeType::Shrinking
        } else {
            VMRemapSizeType::Growing
        }
    }
}

#[derive(Debug)]
pub struct VMRemapOptions {
    old_addr: usize,
    old_size: usize,
    new_size: usize,
    flags: MRemapFlags,
}

impl VMRemapOptions {
    pub fn new(
        old_addr: usize,
        old_size: usize,
        new_size: usize,
        flags: MRemapFlags,
    ) -> Result<Self> {
        let old_addr = if old_addr % PAGE_SIZE != 0 {
            return_errno!(EINVAL, "unaligned old address");
        } else {
            old_addr
        };
        let old_size = if old_size == 0 {
            // TODO: support old_size is zero for shareable mapping
            warn!("do not support old_size is zero");
            return_errno!(EINVAL, "invalid old size");
        } else {
            align_up(old_size, PAGE_SIZE)
        };
        if let Some(new_addr) = flags.new_addr() {
            if new_addr % PAGE_SIZE != 0 {
                return_errno!(EINVAL, "unaligned new address");
            }
        }
        let new_size = if new_size == 0 {
            return_errno!(EINVAL, "invalid new size");
        } else {
            align_up(new_size, PAGE_SIZE)
        };
        Ok(Self {
            old_addr,
            old_size,
            new_size,
            flags,
        })
    }

    pub fn old_addr(&self) -> usize {
        self.old_addr
    }

    pub fn old_size(&self) -> usize {
        self.old_size
    }

    pub fn new_size(&self) -> usize {
        self.new_size
    }

    pub fn flags(&self) -> MRemapFlags {
        self.flags
    }
}

#[derive(Debug)]
pub struct VMRemapResult {
    mmap_options: Option<VMMapOptions>,
    // For RemapFlags::MayMove and size is growing case:
    // If mmap_result_addr is None, we need to do mmap and unmap the old range.
    // If not None, then addr is specified, and thus it just mmap after the old range and should be no munmap.
    mmap_result_addr: Option<usize>,
    munmap_args: Option<(usize, usize)>, // (munmap_addr, munmap_size)
    // There is no lock between parsing mremap options and do the mmap/munmap. If RemapFlags::MayMove is specified,
    // when parsing the mremap options, there could be enough free space for desired address and space. But when doing
    // the actual mmap, the free space could be used by other threads or processes. In this case, check this element.
    // If true, mmap should be done again.
    may_move: bool,
}

impl VMRemapResult {
    pub fn new(
        mmap_options: Option<VMMapOptions>,
        mmap_result_addr: Option<usize>,
        munmap_args: Option<(usize, usize)>,
        may_move: bool,
    ) -> Self {
        Self {
            mmap_options,
            mmap_result_addr,
            munmap_args,
            may_move,
        }
    }

    pub fn mmap_options(&self) -> &Option<VMMapOptions> {
        &self.mmap_options
    }

    pub fn mmap_result_addr(&self) -> &Option<usize> {
        &self.mmap_result_addr
    }

    pub fn munmap_args(&self) -> &Option<(usize, usize)> {
        &self.munmap_args
    }

    pub fn may_move(&self) -> bool {
        self.may_move
    }
}

pub trait VMRemapParser {
    fn parse(&self, options: &VMRemapOptions, vma: &VMArea) -> Result<VMRemapResult> {
        let old_addr = options.old_addr();
        let old_size = options.old_size();
        let old_range = VMRange::new_with_size(old_addr, old_size)?;
        let new_size = options.new_size();
        let flags = options.flags();
        let size_type = VMRemapSizeType::new(&old_size, &new_size);

        // Get the memory permissions of the old range
        let perms = vma.perms();
        // Get the write back file of the old range if there is one.
        let writeback_file = vma.writeback_file();

        // FIXME: Current implementation for file-backed memory mremap has limitation that if a SUBRANGE of the previous
        // file-backed mmap with MAP_SHARED is then mremap-ed with MREMAP_MAYMOVE, there will be two vmas that have the same backed file.
        // For Linux, writing to either memory vma or the file will update the other two equally. But we won't be able to support this before
        // we really have paging. Thus, if the old_range is not equal to a recorded vma, we will just return with error.
        if writeback_file.is_some() && &old_range != vma.range() {
            return_errno!(EINVAL, "Known limition")
        }

        // Implement mremap as one optional mmap followed by one optional munmap.
        //
        // The exact arguments for the mmap and munmap are determined by the values of MRemapFlags,
        // SizeType and writeback_file. There is a total of 18 combinations among MRemapFlags and
        // SizeType and writeback_file. As some combinations result in the same mmap and munmap operations,
        // the following code only needs to match below patterns of (MRemapFlags, SizeType, writeback_file)
        // and treat each case accordingly.

        // Determine whether need to do mmap. And when possible, determine the returned address
        let (need_mmap, mut ret_addr) = match (flags, size_type, writeback_file) {
            (MRemapFlags::None, VMRemapSizeType::Growing, None) => {
                let mmap_opts = VMMapOptionsBuilder::default()
                    .size(new_size - old_size)
                    .addr(VMMapAddr::Need(old_range.end()))
                    .perms(perms)
                    .initializer(VMInitializer::DoNothing())
                    .build()?;
                let ret_addr = Some(old_addr);
                (Some(mmap_opts), ret_addr)
            }
            (MRemapFlags::None, VMRemapSizeType::Growing, Some((backed_file, offset))) => {
                // Update writeback file offset
                let new_writeback_file = Some((backed_file.clone(), offset + vma.size()));
                let vm_initializer_for_new_range = VMInitializer::LoadFromFile {
                    file: backed_file.clone(),
                    offset: offset + vma.size(), // file-backed mremap should start from the end of previous mmap/mremap file
                };
                let mmap_opts = VMMapOptionsBuilder::default()
                    .size(new_size - old_size)
                    .addr(VMMapAddr::Need(old_range.end()))
                    .perms(perms)
                    .initializer(vm_initializer_for_new_range)
                    .writeback_file(new_writeback_file)
                    .build()?;
                let ret_addr = Some(old_addr);
                (Some(mmap_opts), ret_addr)
            }
            (MRemapFlags::MayMove, VMRemapSizeType::Growing, None) => {
                let prefered_new_range =
                    VMRange::new_with_size(old_addr + old_size, new_size - old_size)?;
                if self.is_free_range(&prefered_new_range) {
                    // Don't need to move the old range
                    let mmap_ops = VMMapOptionsBuilder::default()
                        .size(prefered_new_range.size())
                        .addr(VMMapAddr::Need(prefered_new_range.start()))
                        .perms(perms)
                        .initializer(VMInitializer::DoNothing())
                        .build()?;
                    (Some(mmap_ops), Some(old_addr))
                } else {
                    // Need to move old range to a new range and init the new range
                    let vm_initializer_for_new_range = VMInitializer::CopyFrom { range: old_range };
                    let mmap_ops = VMMapOptionsBuilder::default()
                        .size(new_size)
                        .addr(VMMapAddr::Any)
                        .perms(perms)
                        .initializer(vm_initializer_for_new_range)
                        .build()?;
                    // Cannot determine the returned address for now, which can only be obtained after calling mmap
                    let ret_addr = None;
                    (Some(mmap_ops), ret_addr)
                }
            }
            (MRemapFlags::MayMove, VMRemapSizeType::Growing, Some((backed_file, offset))) => {
                let prefered_new_range =
                    VMRange::new_with_size(old_addr + old_size, new_size - old_size)?;
                if self.is_free_range(&prefered_new_range) {
                    // Don't need to move the old range
                    let vm_initializer_for_new_range = VMInitializer::LoadFromFile {
                        file: backed_file.clone(),
                        offset: offset + vma.size(), // file-backed mremap should start from the end of previous mmap/mremap file
                    };
                    // Write back file should start from new offset
                    let new_writeback_file = Some((backed_file.clone(), offset + vma.size()));
                    let mmap_ops = VMMapOptionsBuilder::default()
                        .size(prefered_new_range.size())
                        .addr(VMMapAddr::Need(prefered_new_range.start()))
                        .perms(perms)
                        .initializer(vm_initializer_for_new_range)
                        .writeback_file(new_writeback_file)
                        .build()?;
                    (Some(mmap_ops), Some(old_addr))
                } else {
                    // Need to move old range to a new range and init the new range
                    let vm_initializer_for_new_range = {
                        let copy_end = vma.end();
                        let copy_range = VMRange::new(old_range.start(), copy_end)?;
                        let reread_file_start_offset = copy_end - vma.start();
                        VMInitializer::CopyOldAndReadNew {
                            old_range: copy_range,
                            file: backed_file.clone(),
                            offset: reread_file_start_offset,
                        }
                    };
                    let new_writeback_file = Some((backed_file.clone(), *offset));
                    let mmap_ops = VMMapOptionsBuilder::default()
                        .size(new_size)
                        .addr(VMMapAddr::Any)
                        .perms(perms)
                        .initializer(vm_initializer_for_new_range)
                        .writeback_file(new_writeback_file)
                        .build()?;
                    // Cannot determine the returned address for now, which can only be obtained after calling mmap
                    let ret_addr = None;
                    (Some(mmap_ops), ret_addr)
                }
            }
            (MRemapFlags::FixedAddr(new_addr), _, None) => {
                let vm_initializer_for_new_range = { VMInitializer::CopyFrom { range: old_range } };
                let mmap_opts = VMMapOptionsBuilder::default()
                    .size(new_size)
                    .addr(VMMapAddr::Force(new_addr))
                    .perms(perms)
                    .initializer(vm_initializer_for_new_range)
                    .build()?;
                let ret_addr = Some(new_addr);
                (Some(mmap_opts), ret_addr)
            }
            (MRemapFlags::FixedAddr(new_addr), _, Some((backed_file, offset))) => {
                let vm_initializer_for_new_range = {
                    let copy_end = vma.end();
                    let copy_range = VMRange::new(old_range.start(), copy_end)?;
                    let reread_file_start_offset = copy_end - vma.start();
                    VMInitializer::CopyOldAndReadNew {
                        old_range: copy_range,
                        file: backed_file.clone(),
                        offset: reread_file_start_offset,
                    }
                };
                let new_writeback_file = Some((backed_file.clone(), *offset));
                let mmap_opts = VMMapOptionsBuilder::default()
                    .size(new_size)
                    .addr(VMMapAddr::Force(new_addr))
                    .perms(perms)
                    .initializer(vm_initializer_for_new_range)
                    .writeback_file(new_writeback_file)
                    .build()?;
                let ret_addr = Some(new_addr);
                (Some(mmap_opts), ret_addr)
            }
            _ => (None, Some(old_addr)),
        };

        let need_munmap = match (flags, size_type) {
            (MRemapFlags::None, VMRemapSizeType::Shrinking)
            | (MRemapFlags::MayMove, VMRemapSizeType::Shrinking) => {
                let unmap_addr = old_addr + new_size;
                let unmap_size = old_size - new_size;
                Some((unmap_addr, unmap_size))
            }
            (MRemapFlags::MayMove, VMRemapSizeType::Growing) => {
                if ret_addr.is_none() {
                    // We must need to do mmap. Thus unmap the old range
                    Some((old_addr, old_size))
                } else {
                    // We must choose to reuse the old range. Thus, no need to unmap
                    None
                }
            }
            (MRemapFlags::FixedAddr(new_addr), _) => {
                let new_range = VMRange::new_with_size(new_addr, new_size)?;
                if new_range.overlap_with(&old_range) {
                    return_errno!(EINVAL, "new range cannot overlap with the old one");
                }
                Some((old_addr, old_size))
            }
            _ => None,
        };

        let may_move = if let MRemapFlags::MayMove = flags {
            true
        } else {
            false
        };
        Ok(VMRemapResult::new(
            need_mmap,
            ret_addr,
            need_munmap,
            may_move,
        ))
    }

    fn is_free_range(&self, request_range: &VMRange) -> bool;
}
