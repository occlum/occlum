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
                let len = file
                    .as_inode_file()
                    .unwrap()
                    .read_at(*offset, buf)
                    .map_err(|_| errno!(EACCES, "failed to init memory from file"))?;
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
                    .as_inode_file()
                    .unwrap()
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
        // TODO: support async I/O in vm code
        self.check_files_are_inodes()?;

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

    fn check_files_are_inodes(&self) -> Result<()> {
        if let Some(VMInitializer::LoadFromFile { file, .. }) = self.initializer.as_ref() {
            if file.as_inode_file().is_none() {
                return_errno!(ENODEV, "VMA must be backed by inode files");
            }
        }
        if let Some(Some((file, _))) = self.writeback_file.as_ref() {
            if file.as_inode_file().is_none() {
                return_errno!(ENODEV, "VMA must be backed by inode files");
            }
        }
        Ok(())
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
pub enum SizeType {
    Same,
    Shrinking,
    Growing,
}

impl SizeType {
    pub fn new(old_size: &usize, new_size: &usize) -> Self {
        if new_size == old_size {
            SizeType::Same
        } else if new_size < old_size {
            SizeType::Shrinking
        } else {
            SizeType::Growing
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
