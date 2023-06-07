use crate::metadata::*;
use crate::prelude::*;
use crate::storage::Storage;
use crate::utils::{AsBuf, Dirty, InodeCache};

use async_trait::async_trait;
use async_vfs::{AsyncFileSystem, AsyncInode};
use bitvec::prelude::*;
use block_device::{BlockDeviceAsFile, BlockRangeIter};
use std::any::Any;
use std::fmt::{Debug, Formatter};
use std::mem::MaybeUninit;
use std::{
    string::String,
    sync::{Arc, Weak},
    vec,
};

/// Async Simple Filesystem
pub struct AsyncSimpleFS {
    /// describe the metadata of fs
    super_block: AsyncRwLock<Dirty<SuperBlock>>,
    /// describe the allocation of blocks
    alloc_map: AsyncRwLock<Dirty<BlockAllocMap>>,
    /// cached inodes
    inodes: AsyncRwLock<InodeCache>,
    /// underlying storage
    storage: Storage,
    /// pointer to self, used by inodes
    self_ptr: Weak<AsyncSimpleFS>,
}

impl AsyncSimpleFS {
    /// Create a new fs on blank block device
    pub async fn create(device: Arc<dyn BlockDeviceAsFile>) -> Result<Arc<Self>> {
        let blocks = if device.total_bytes() / BLOCK_SIZE > MAX_NBLOCKS {
            warn!(
                "device size is too big, use first {:#x} blocks",
                MAX_NBLOCKS
            );
            MAX_NBLOCKS
        } else {
            device.total_bytes() / BLOCK_SIZE
        };
        if blocks < MIN_NBLOCKS {
            return_errno!(EINVAL, "device size is too small");
        }
        let alloc_map_blocks = (blocks + BLKBITS - 1) / BLKBITS;

        let super_block = SuperBlock {
            magic: FS_MAGIC,
            blocks: blocks as u32,
            unused_blocks: (blocks - BLKN_FREEMAP.to_raw() as usize - alloc_map_blocks) as u32,
            info: Str32::from(FS_INFO),
            alloc_map_blocks: alloc_map_blocks as u32,
        };
        let alloc_map = {
            let mut bitset = BitVec::with_capacity(alloc_map_blocks * BLKBITS);
            bitset.extend(core::iter::repeat(false).take(alloc_map_blocks * BLKBITS));
            for i in 0..(BLKN_FREEMAP.to_raw() as usize) + alloc_map_blocks {
                bitset.set(i, true);
            }
            BlockAllocMap::from_bitset(bitset)
        };

        let sfs = Self {
            super_block: AsyncRwLock::new(Dirty::new_dirty(super_block)),
            alloc_map: AsyncRwLock::new(Dirty::new_dirty(alloc_map)),
            inodes: AsyncRwLock::new(InodeCache::new(INODE_CACHE_CAP)),
            storage: Storage::new(device),
            self_ptr: Weak::default(),
        }
        .wrap();

        // Init the root inode
        let root = sfs
            ._new_inode(BLKN_ROOT, Dirty::new_dirty(DiskInode::new_dir()))
            .await;
        root.inner.write().await.init_direntry(BLKN_ROOT).await?;
        root.sync_all().await?;
        sfs.sync_metadata().await?;

        Ok(sfs)
    }

    /// Load fs from an existing block device
    pub async fn open(device: Arc<dyn BlockDeviceAsFile>) -> Result<Arc<Self>> {
        let device_storage = Storage::new(device);
        // Load the super_block
        let super_block = device_storage
            .load_struct::<SuperBlock>(BLKN_SUPER, 0)
            .await?;
        super_block.validate()?;
        // Load the alloc_map
        let alloc_map = {
            let mut alloc_map_disk = vec![0u8; BLOCK_SIZE * super_block.alloc_map_blocks as usize];
            device_storage
                .read_at(BLKN_FREEMAP, alloc_map_disk.as_mut_slice(), 0)
                .await?;
            let alloc_map_bitset = BitVec::from(alloc_map_disk.as_slice());
            BlockAllocMap::from_bitset(alloc_map_bitset)
        };
        Ok(Self {
            super_block: AsyncRwLock::new(Dirty::new(super_block)),
            alloc_map: AsyncRwLock::new(Dirty::new(alloc_map)),
            inodes: AsyncRwLock::new(InodeCache::new(INODE_CACHE_CAP)),
            storage: device_storage,
            self_ptr: Weak::default(),
        }
        .wrap())
    }

    /// Wrap pure AsyncSimpleFS with Arc
    /// Private used in constructors
    fn wrap(self) -> Arc<Self> {
        // Create an Arc, make a Weak from it, then put it into the struct.
        // It's a little tricky.
        let fs = Arc::new(self);
        let weak = Arc::downgrade(&fs);
        let ptr = Arc::into_raw(fs) as *mut Self;
        unsafe {
            (*ptr).self_ptr = weak;
        }
        unsafe { Arc::from_raw(ptr) }
    }

    /// Allocate a free block, return block id
    async fn alloc_block(&self) -> Option<Bid> {
        let mut alloc_map = self.alloc_map.write().await;
        let id = alloc_map.alloc();
        if let Some(block_id) = id {
            let mut super_block = self.super_block.write().await;
            if super_block.unused_blocks == 0 {
                alloc_map.free(block_id);
                return None;
            }
            // unused_blocks will not underflow
            super_block.unused_blocks -= 1;
            // trace!("alloc block {:#x}", block_id);
        }
        id.map(|id| Bid::new(id as _))
    }

    /// Free a block
    async fn free_block(&self, block_id: Bid) {
        let mut alloc_map = self.alloc_map.write().await;
        let mut super_block = self.super_block.write().await;
        assert!(alloc_map.is_allocated(block_id.to_raw() as _));
        alloc_map.free(block_id.to_raw() as _);
        super_block.unused_blocks += 1;
        // trace!("free block {:#x}", block_id);
        // clear the block
        self.storage.write_at(block_id, &ZEROS, 0).await.unwrap();
    }

    /// Create a new inode struct, and insert into inode cache
    /// Private used for load or create inode
    async fn _new_inode(&self, id: InodeId, disk_inode: Dirty<DiskInode>) -> Arc<Inode> {
        let inode = {
            let inode_inner = InodeInner {
                id,
                disk_inode,
                is_freed: false,
                fs: self.self_ptr.clone(),
            };
            Arc::new(Inode::new(inode_inner, Extension::new()))
        };
        if let Some((_, lru_inode)) = self.inodes.write().await.push(id, inode.clone()) {
            lru_inode.sync_all().await.unwrap();
        }
        inode
    }

    /// Get inode by id. Load if not in memory.
    /// ** Must ensure it's a valid inode **
    async fn get_inode(&self, id: InodeId) -> Arc<Inode> {
        assert!(self.alloc_map.read().await.is_allocated(id.to_raw() as _));

        // In the cache
        let mut inode_cache = self.inodes.write().await;
        if let Some(inode) = inode_cache.get(&id) {
            return inode.clone();
        }
        // Load if not in cache
        let disk_inode = self.storage.load_struct::<DiskInode>(id, 0).await.unwrap();
        let inode = {
            let inode_inner = InodeInner {
                id,
                disk_inode: Dirty::new(disk_inode),
                is_freed: false,
                fs: self.self_ptr.clone(),
            };
            Arc::new(Inode::new(inode_inner, Extension::new()))
        };
        if let Some((_, lru_inode)) = inode_cache.push(id, inode.clone()) {
            lru_inode.sync_all().await.unwrap();
        }
        inode
    }

    /// Create a new inode file
    async fn new_inode_file(&self) -> Result<Arc<Inode>> {
        let id = self
            .alloc_block()
            .await
            .ok_or(errno!(EIO, "no device space"))?;
        let disk_inode = Dirty::new_dirty(DiskInode::new_file());
        Ok(self._new_inode(id, disk_inode).await)
    }

    /// Create a new inode symlink
    async fn new_inode_symlink(&self) -> Result<Arc<Inode>> {
        let id = self
            .alloc_block()
            .await
            .ok_or(errno!(EIO, "no device space"))?;
        let disk_inode = Dirty::new_dirty(DiskInode::new_symlink());
        Ok(self._new_inode(id, disk_inode).await)
    }

    /// Create a new inode dir
    async fn new_inode_dir(&self, parent: InodeId) -> Result<Arc<Inode>> {
        let id = self
            .alloc_block()
            .await
            .ok_or(errno!(EIO, "no device space"))?;
        let disk_inode = Dirty::new_dirty(DiskInode::new_dir());
        let inode = self._new_inode(id, disk_inode).await;
        if let Err(e) = inode.inner.write().await.init_direntry(parent).await {
            // rollback
            inode.inner.write().await._resize(0).await?;
            self.inodes.write().await.pop(&id);
            self.free_block(id).await;
            return Err(e);
        }
        Ok(inode)
    }

    /// Flush all the inodes and metadata, then commit to underlying storage for durability
    async fn sync_all(&self) -> Result<()> {
        // writeback cached inodes
        self.sync_cached_inodes().await?;
        // writeback alloc_map and super_block
        self.sync_metadata().await?;
        // Sync the data in device
        self.storage.sync().await?;
        Ok(())
    }

    /// Flush the super_block and alloc_map
    async fn sync_metadata(&self) -> Result<()> {
        let alloc_map_dirty = self.alloc_map.read().await.is_dirty();
        let super_block_dirty = self.super_block.read().await.is_dirty();
        if alloc_map_dirty {
            let mut alloc_map = self.alloc_map.write().await;
            self.storage
                .write_at(BLKN_FREEMAP, alloc_map.as_buf(), 0)
                .await?;
            alloc_map.sync();
        }
        if super_block_dirty {
            let mut super_block = self.super_block.write().await;
            self.storage
                .store_struct::<SuperBlock>(BLKN_SUPER, 0, &super_block)
                .await?;
            super_block.sync();
        }
        Ok(())
    }

    /// Flush the cached dirty inodes
    async fn sync_cached_inodes(&self) -> Result<()> {
        let mut inodes_cache = self.inodes.write().await;
        let inodes = inodes_cache.retain_items(|i| Arc::strong_count(&i) > 1);
        for inode in inodes.iter() {
            inode.sync_all().await?;
        }
        Ok(())
    }
}

#[async_trait]
impl AsyncFileSystem for AsyncSimpleFS {
    async fn sync(&self) -> Result<()> {
        self.sync_all().await
    }

    async fn root_inode(&self) -> Arc<dyn AsyncInode> {
        let inode = self.get_inode(BLKN_ROOT).await;
        inode
    }

    async fn info(&self) -> FsInfo {
        let sb = self.super_block.read().await;
        FsInfo {
            magic: sb.magic as usize,
            bsize: BLOCK_SIZE,
            frsize: BLOCK_SIZE,
            blocks: sb.blocks as usize,
            bfree: sb.unused_blocks as usize,
            bavail: sb.unused_blocks as usize,
            files: sb.blocks as usize,        // inaccurate
            ffree: sb.unused_blocks as usize, // inaccurate
            namemax: MAX_FNAME_LEN,
        }
    }
}

/// Inode for AsyncSimpleFS
pub struct Inode {
    /// Inner inode
    inner: AsyncRwLock<InodeInner>,
    /// Reference to fs
    fs: Weak<AsyncSimpleFS>,
    /// Extensions for Inode, e.g., flock
    ext: Extension,
}

impl Inode {
    pub(crate) fn new(inner: InodeInner, ext: Extension) -> Self {
        Self {
            fs: inner.fs.clone(),
            inner: AsyncRwLock::new(inner),
            ext,
        }
    }
}

impl Debug for Inode {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "Inode {:?}", self.inner)
    }
}

#[async_trait]
impl AsyncInode for Inode {
    async fn read_at(&self, offset: usize, buf: &mut [u8]) -> Result<usize> {
        let inner = self.inner.read().await;
        let len = match inner.disk_inode.type_ {
            FileType::File | FileType::SymLink => inner._read_at(offset, buf).await?,
            _ => return_errno!(EISDIR, "not file"),
        };
        Ok(len)
    }

    async fn write_at(&self, offset: usize, buf: &[u8]) -> Result<usize> {
        let inner = self.inner.read().await;
        let len = match inner.disk_inode.type_ {
            FileType::File | FileType::SymLink => {
                let end_offset = offset + buf.len();
                if end_offset > inner.disk_inode.size as usize {
                    drop(inner);
                    let mut inner_mut = self.inner.write().await;
                    // When we get the lock, the file size may be changed
                    if end_offset > inner_mut.disk_inode.size as usize {
                        inner_mut._resize(end_offset).await?;
                    }
                    inner_mut._write_at(offset, buf).await?
                } else {
                    inner._write_at(offset, buf).await?
                }
            }
            _ => return_errno!(EISDIR, "not file"),
        };
        Ok(len)
    }

    /// the size returned here is logical size(entry num for directory), not the disk space used.
    async fn metadata(&self) -> Result<Metadata> {
        let inner = self.inner.read().await;
        let disk_inode = &inner.disk_inode;
        Ok(Metadata {
            dev: 0,
            rdev: 0,
            inode: inner.id.to_raw() as _,
            size: match disk_inode.type_ {
                FileType::File | FileType::SymLink | FileType::Dir => disk_inode.size as usize,
                FileType::CharDevice => 0,
                FileType::BlockDevice => 0,
                FileType::NamedPipe => 0,
                FileType::Socket => 0,
            },
            mode: 0o777,
            type_: VfsFileType::from(disk_inode.type_.clone()),
            blocks: disk_inode.blocks as usize * (BLOCK_SIZE / 512), // Number of 512B blocks
            atime: Timespec { sec: 0, nsec: 0 },
            mtime: Timespec { sec: 0, nsec: 0 },
            ctime: Timespec { sec: 0, nsec: 0 },
            nlinks: disk_inode.nlinks as usize,
            uid: 0,
            gid: 0,
            blk_size: BLOCK_SIZE,
        })
    }

    async fn set_metadata(&self, _metadata: &Metadata) -> Result<()> {
        // TODO: support to modify the metadata
        Ok(())
    }

    async fn sync_all(&self) -> Result<()> {
        self.inner.read().await.sync_data().await?;
        if self.inner.read().await.is_dirty() {
            self.inner.write().await.sync_metadata().await?;
        }
        Ok(())
    }

    async fn sync_data(&self) -> Result<()> {
        self.inner.read().await.sync_data().await
    }

    async fn resize(&self, len: usize) -> Result<()> {
        let inode_type = self.inner.read().await.disk_inode.type_;
        match inode_type {
            FileType::File | FileType::SymLink => {
                let mut inner_mut = self.inner.write().await;
                inner_mut._resize(len).await?
            }
            _ => return_errno!(EISDIR, "not file"),
        }
        Ok(())
    }

    async fn fallocate(&self, mode: &FallocateMode, offset: usize, len: usize) -> Result<()> {
        let inode_type = self.inner.read().await.disk_inode.type_;
        if inode_type != FileType::File && inode_type != FileType::Dir {
            return_errno!(ENODEV, "not a regular file or directory");
        }
        let range = {
            let end_offset = offset
                .checked_add(len)
                .ok_or(errno!(EFBIG, "too big size"))?;
            if end_offset > MAX_FILE_SIZE {
                return_errno!(EFBIG, "too big size");
            }
            (offset, end_offset)
        };

        match mode {
            FallocateMode::Allocate(flags) if flags.is_empty() => {
                let file_size = self.inner.read().await.disk_inode.size;
                if range.1 > file_size as usize {
                    let mut inner_mut = self.inner.write().await;
                    // When we get the lock, the file size may be changed
                    if range.1 > inner_mut.disk_inode.size as usize {
                        inner_mut._resize(range.1).await?;
                    }
                }
            }
            _ => {
                warn!("only support posix_fallocate now");
            }
        }
        Ok(())
    }

    async fn create(
        &self,
        name: &str,
        type_: VfsFileType,
        _mode: u16,
    ) -> Result<Arc<dyn AsyncInode>> {
        let info = self.inner.read().await.disk_inode.clone();
        if info.type_ != FileType::Dir {
            return_errno!(ENOTDIR, "not dir");
        }

        // Fast path to return error
        if info.nlinks == 0 {
            return_errno!(ENOENT, "dir removed");
        }
        if name.len() > MAX_FNAME_LEN {
            return_errno!(ENAMETOOLONG, "file name too long");
        }
        if self.find(name).await.is_ok() {
            return_errno!(EEXIST, "entry exist");
        }

        // Normal path
        let mut inner_mut = self.inner.write().await;
        if inner_mut.disk_inode.nlinks == 0 {
            return_errno!(ENOENT, "dir removed");
        }
        // When we get the lock, the entry may be exist
        if inner_mut.get_file_inode_id(name).await.is_some() {
            return_errno!(EEXIST, "entry exist");
        }
        // Create new inode
        let inode = {
            let fs = inner_mut.fs();
            match type_ {
                VfsFileType::File | VfsFileType::Socket => fs.new_inode_file().await?,
                VfsFileType::SymLink => fs.new_inode_symlink().await?,
                VfsFileType::Dir => fs.new_inode_dir(inner_mut.id).await?,
                _ => return_errno!(EINVAL, "invalid type"),
            }
        };

        // Write new entry
        if let Err(e) = inner_mut
            .append_direntry(&DiskDirEntry {
                id: inode.inner.read().await.id.to_raw() as u32,
                name: Str256::from(name),
                type_: FileType::from(type_) as u32,
            })
            .await
        {
            // rollback
            let mut child_inner_mut = inode.inner.write().await;
            child_inner_mut.dec_nlinks();
            if type_ == VfsFileType::Dir {
                child_inner_mut.dec_nlinks();
            }
            return Err(e);
        }
        if type_ == VfsFileType::Dir {
            inner_mut.inc_nlinks(); //for ..
        }

        Ok(inode)
    }

    async fn link(&self, name: &str, other: &Arc<dyn AsyncInode>) -> Result<()> {
        let info = self.inner.read().await.disk_inode.clone();
        if info.type_ != FileType::Dir {
            return_errno!(ENOTDIR, "not dir");
        }

        // Fast path to return error
        if info.nlinks == 0 {
            return_errno!(ENOENT, "dir removed");
        }
        if name.len() > MAX_FNAME_LEN {
            return_errno!(ENAMETOOLONG, "file name too long");
        }
        if self.find(name).await.is_ok() {
            return_errno!(EEXIST, "entry exist");
        }
        let other = other
            .downcast_ref::<Inode>()
            .ok_or(errno!(EXDEV, "not same fs"))?;
        if !Arc::ptr_eq(&self.fs(), &other.fs()) {
            return_errno!(EXDEV, "not same fs");
        }
        if other.inner.read().await.disk_inode.type_ == FileType::Dir {
            return_errno!(EISDIR, "entry is dir");
        }

        // Normal path
        let (mut self_inner_mut, mut other_inner_mut) = write_lock_two_inodes(self, other).await;
        if self_inner_mut.disk_inode.nlinks == 0 {
            return_errno!(ENOENT, "dir removed");
        }
        // When we get the lock, the entry may be exist
        if self_inner_mut.get_file_inode_id(name).await.is_some() {
            return_errno!(EEXIST, "entry exist");
        }
        self_inner_mut
            .append_direntry(&DiskDirEntry {
                id: other_inner_mut.id.to_raw() as u32,
                name: Str256::from(name),
                type_: other_inner_mut.disk_inode.type_ as u32,
            })
            .await?;
        other_inner_mut.inc_nlinks();
        Ok(())
    }

    async fn unlink(&self, name: &str) -> Result<()> {
        let info = self.inner.read().await.disk_inode.clone();
        if info.type_ != FileType::Dir {
            return_errno!(ENOTDIR, "self is not dir");
        }
        if name == ".." {
            return_errno!(ENOTEMPTY, ".. is not empty");
        }
        if name == "." || name.is_empty() {
            return_errno!(EINVAL, "cannot unlink self");
        }
        if name.len() > MAX_FNAME_LEN {
            return_errno!(ENAMETOOLONG, "file name too long");
        }
        if info.nlinks == 0 {
            return_errno!(ENOENT, "dir removed");
        }

        let mut this_inner_mut = self.inner.write().await;
        if this_inner_mut.disk_inode.nlinks == 0 {
            return_errno!(ENOENT, "dir removed");
        }
        // When we get the lock, the entry may be removed
        let (inode_id, type_, entry_id) = this_inner_mut
            .get_file_inode_and_entry_id(name)
            .await
            .ok_or(errno!(ENOENT, "not found"))?;
        let child = this_inner_mut.fs().get_inode(inode_id).await;
        if type_ == FileType::Dir {
            if child.inner.read().await.direntry_cnt() > 2 {
                return_errno!(ENOTEMPTY, "dir not empty");
            }
            // for ".."
            this_inner_mut.dec_nlinks();
        }
        this_inner_mut.remove_direntry(entry_id).await?;
        drop(this_inner_mut);

        let mut child_inner_mut = child.inner.write().await;
        child_inner_mut.dec_nlinks();
        if type_ == FileType::Dir {
            // for "."
            child_inner_mut.dec_nlinks();
        }

        Ok(())
    }

    async fn move_(
        &self,
        old_name: &str,
        target: &Arc<dyn AsyncInode>,
        new_name: &str,
    ) -> Result<()> {
        let self_info = self.inner.read().await.disk_inode.clone();
        if self_info.type_ != FileType::Dir {
            return_errno!(ENOTDIR, "self is not dir");
        }
        if self_info.nlinks == 0 {
            return_errno!(ENOENT, "dir removed");
        }
        if old_name == "." || old_name == ".." || old_name.is_empty() {
            return_errno!(EISDIR, "old name is dir");
        }
        if new_name == "." || new_name == ".." || new_name.is_empty() {
            return_errno!(EISDIR, "new name is dir");
        }
        if old_name.len() > MAX_FNAME_LEN || new_name.len() > MAX_FNAME_LEN {
            return_errno!(ENAMETOOLONG, "old_name/new_name too long");
        }

        let dest = target
            .downcast_ref::<Inode>()
            .ok_or(errno!(EXDEV, "not same fs"))?;
        let dest_info = dest.inner.read().await.disk_inode.clone();
        if !Arc::ptr_eq(&self.fs(), &dest.fs()) {
            return_errno!(EXDEV, "not same fs");
        }
        if dest_info.type_ != FileType::Dir {
            return_errno!(ENOTDIR, "dest not dir");
        }
        if dest_info.nlinks == 0 {
            return_errno!(ENOENT, "dest dir removed");
        }

        // Fast path to return error
        let inode = self.find(old_name).await?;
        let inode = inode.downcast_ref::<Inode>().unwrap();
        // Avoid deadlock
        if inode.inner.read().await.id == dest.inner.read().await.id {
            return_errno!(EINVAL, "invalid path");
        }

        // Normal path
        if self.inner.read().await.id == dest.inner.read().await.id {
            // Rename in one dir
            let mut inner_mut = self.inner.write().await;
            if inner_mut.disk_inode.nlinks == 0 {
                return_errno!(ENOENT, "dir removed");
            }
            // When we get the lock, the entry may be removed
            let (inode_id, inode_type, entry_id) = inner_mut
                .get_file_inode_and_entry_id(old_name)
                .await
                .ok_or(errno!(ENOENT, "not found"))?;

            // Replace inode
            if let Some((replace_inode_id, replace_inode_type, replace_entry_id)) =
                inner_mut.get_file_inode_and_entry_id(new_name).await
            {
                if inode_id == replace_inode_id {
                    // Same Inode, do nothing
                    return Ok(());
                }
                let replace_inode = inner_mut.fs().get_inode(replace_inode_id).await;
                match (inode_type, replace_inode_type) {
                    (FileType::Dir, FileType::Dir) => {
                        if replace_inode.inner.read().await.direntry_cnt() > 2 {
                            return_errno!(ENOTEMPTY, "dir not empty");
                        }
                    }
                    (FileType::Dir, _) => {
                        return_errno!(ENOTDIR, "not dir");
                    }
                    (_, FileType::Dir) => {
                        return_errno!(EISDIR, "entry is dir");
                    }
                    _ => {}
                }
                if replace_inode_type == FileType::Dir {
                    // for ".."
                    inner_mut.dec_nlinks();
                }
                inner_mut
                    .write_direntry(
                        entry_id,
                        &DiskDirEntry {
                            id: inode_id.to_raw() as u32,
                            name: Str256::from(new_name),
                            type_: inode_type as u32,
                        },
                    )
                    .await;
                // this operation may change the entry_id, put it after write_direntry
                inner_mut.remove_direntry(replace_entry_id).await?;
                drop(inner_mut);

                let mut replace_inode_inner_mut = replace_inode.inner.write().await;
                replace_inode_inner_mut.dec_nlinks();
                if replace_inode_type == FileType::Dir {
                    // for "."
                    replace_inode_inner_mut.dec_nlinks();
                }
            } else {
                // just modify name
                inner_mut
                    .write_direntry(
                        entry_id,
                        &DiskDirEntry {
                            id: inode_id.to_raw() as u32,
                            name: Str256::from(new_name),
                            type_: inode_type as u32,
                        },
                    )
                    .await;
            }
        } else {
            // Move between dirs
            let (mut self_inner_mut, mut dest_inner_mut) = write_lock_two_inodes(self, dest).await;
            if self_inner_mut.disk_inode.nlinks == 0 || dest_inner_mut.disk_inode.nlinks == 0 {
                return_errno!(ENOENT, "dir removed");
            }
            // When we get the lock, the entry may be removed
            let (inode_id, inode_type, entry_id) = self_inner_mut
                .get_file_inode_and_entry_id(old_name)
                .await
                .ok_or(errno!(ENOENT, "not found"))?;

            // Replace inode
            if let Some((replace_inode_id, replace_inode_type, replace_entry_id)) =
                dest_inner_mut.get_file_inode_and_entry_id(new_name).await
            {
                if inode_id == replace_inode_id {
                    // Same Inode, do nothing
                    return Ok(());
                }
                let replace_inode = dest_inner_mut.fs().get_inode(replace_inode_id).await;
                match (inode_type, replace_inode_type) {
                    (FileType::Dir, FileType::Dir) => {
                        if replace_inode.inner.read().await.direntry_cnt() > 2 {
                            return_errno!(ENOTEMPTY, "dir not empty");
                        }
                    }
                    (FileType::Dir, _) => {
                        return_errno!(ENOTDIR, "not dir");
                    }
                    (_, FileType::Dir) => {
                        return_errno!(EISDIR, "entry is dir");
                    }
                    _ => {}
                }
                // Replace the inode in dest.
                // If is dir, no need to update the links in dest
                dest_inner_mut
                    .write_direntry(
                        replace_entry_id,
                        &DiskDirEntry {
                            id: inode_id.to_raw() as u32,
                            name: Str256::from(new_name),
                            type_: inode_type as u32,
                        },
                    )
                    .await;
                drop(dest_inner_mut);
                let mut replace_inode_inner_mut = replace_inode.inner.write().await;
                replace_inode_inner_mut.dec_nlinks();
                if replace_inode_type == FileType::Dir {
                    // for "."
                    replace_inode_inner_mut.dec_nlinks();
                }

                // remove the entry in old dir
                self_inner_mut.remove_direntry(entry_id).await?;
                if inode_type == FileType::Dir {
                    self_inner_mut.dec_nlinks();
                }
            } else {
                // just move inode
                dest_inner_mut
                    .append_direntry(&DiskDirEntry {
                        id: inode_id.to_raw() as u32,
                        name: Str256::from(new_name),
                        type_: inode_type as u32,
                    })
                    .await?;
                self_inner_mut.remove_direntry(entry_id).await?;
                if inode_type == FileType::Dir {
                    self_inner_mut.dec_nlinks();
                    dest_inner_mut.inc_nlinks();
                }
            }
        }
        Ok(())
    }

    async fn find(&self, name: &str) -> Result<Arc<dyn AsyncInode>> {
        let inner = self.inner.read().await;
        if inner.disk_inode.type_ != FileType::Dir {
            return_errno!(ENOTDIR, "not dir");
        }
        if name.len() > MAX_FNAME_LEN {
            return_errno!(ENAMETOOLONG, "file name too long");
        }

        let inode_id = inner
            .get_file_inode_id(name)
            .await
            .ok_or(errno!(ENOENT, "not found"))?;
        Ok(inner.fs().get_inode(inode_id).await)
    }

    async fn iterate_entries(&self, ctx: &mut DirentWriterContext) -> Result<usize> {
        let inner = self.inner.read().await;
        if inner.disk_inode.type_ != FileType::Dir {
            return_errno!(ENOTDIR, "not dir");
        }

        for entry_id in ctx.pos()..inner.disk_inode.size as usize / DIRENT_SIZE {
            let entry = inner.read_direntry(entry_id).await?;
            if ctx
                .write_entry(
                    entry.name.as_ref(),
                    entry.id as u64,
                    VfsFileType::from(FileType::from(entry.type_)),
                )
                .is_err()
            {
                if ctx.written_len() == 0 {
                    return_errno!(EINVAL, "write entry fail");
                } else {
                    break;
                }
            }
        }
        Ok(ctx.written_len())
    }

    fn fs(&self) -> Arc<dyn AsyncFileSystem> {
        self.fs.upgrade().unwrap()
    }

    fn ext(&self) -> Option<&Extension> {
        Some(&self.ext)
    }

    fn as_any_ref(&self) -> &dyn Any {
        self
    }
}

async fn write_lock_two_inodes<'a>(
    this: &'a Inode,
    other: &'a Inode,
) -> (
    AsyncRwLockWriteGuard<'a, InodeInner>,
    AsyncRwLockWriteGuard<'a, InodeInner>,
) {
    if this.inner.read().await.id < other.inner.read().await.id {
        let this = this.inner.write().await;
        let other = other.inner.write().await;
        (this, other)
    } else {
        let other = other.inner.write().await;
        let this = this.inner.write().await;
        (this, other)
    }
}

/// Inner inode for AsyncSimpleFS
pub(crate) struct InodeInner {
    /// Inode number
    id: InodeId,
    /// On-disk Inode
    disk_inode: Dirty<DiskInode>,
    /// whether the block to store disk_inode is freed
    is_freed: bool,
    /// Reference to fs, used by almost all operations
    fs: Weak<AsyncSimpleFS>,
}

impl Debug for InodeInner {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(
            f,
            "InodeInner {{ id: {:?}, disk: {:?} }}",
            self.id, self.disk_inode
        )
    }
}

impl InodeInner {
    fn fs(&self) -> Arc<AsyncSimpleFS> {
        self.fs.upgrade().unwrap()
    }

    /// Map file block id to device block id
    async fn get_device_block_id(&self, file_block_id: Bid) -> Result<Bid> {
        let disk_inode = &self.disk_inode;
        let device_block_id = match file_block_id.to_raw() as usize {
            id if id >= disk_inode.blocks as _ => {
                return_errno!(EINVAL, "invalid file block id");
            }
            id if id < MAX_NBLOCK_DIRECT => disk_inode.direct[id],
            id if id < MAX_NBLOCK_INDIRECT => {
                let device_block_id = self
                    .fs()
                    .storage
                    .load_struct::<u32>(
                        Bid::new(disk_inode.indirect as _),
                        ENTRY_SIZE * (id - NDIRECT),
                    )
                    .await?;
                device_block_id
            }
            id if id < MAX_NBLOCK_DOUBLE_INDIRECT => {
                // double indirect
                let indirect_id = id - MAX_NBLOCK_INDIRECT;
                let indirect_block_id = self
                    .fs()
                    .storage
                    .load_struct::<u32>(
                        Bid::new(disk_inode.db_indirect as _),
                        ENTRY_SIZE * (indirect_id / BLK_NENTRY),
                    )
                    .await?;
                assert!(indirect_block_id > 0);
                let device_block_id = self
                    .fs()
                    .storage
                    .load_struct::<u32>(
                        Bid::new(indirect_block_id as _),
                        ENTRY_SIZE * (indirect_id as usize % BLK_NENTRY),
                    )
                    .await?;
                assert!(device_block_id > 0);
                device_block_id
            }
            _ => unimplemented!("triple indirect blocks is not supported"),
        };
        Ok(Bid::new(device_block_id as _))
    }

    /// Set the device block id for the file block id
    async fn set_device_block_id(
        &mut self,
        file_block_id: Bid,
        device_block_id: Bid,
    ) -> Result<()> {
        match file_block_id.to_raw() as usize {
            id if id >= self.disk_inode.blocks as _ => {
                return_errno!(EINVAL, "invalid file block id");
            }
            id if id < MAX_NBLOCK_DIRECT => {
                self.disk_inode.direct[id] = device_block_id.to_raw() as u32;
                Ok(())
            }
            id if id < MAX_NBLOCK_INDIRECT => {
                let device_block_id = device_block_id.to_raw() as u32;
                self.fs()
                    .storage
                    .store_struct::<u32>(
                        Bid::new(self.disk_inode.indirect as _),
                        ENTRY_SIZE * (id - NDIRECT),
                        &device_block_id,
                    )
                    .await?;
                Ok(())
            }
            id if id < MAX_NBLOCK_DOUBLE_INDIRECT => {
                // double indirect
                let indirect_id = id - MAX_NBLOCK_INDIRECT;
                let indirect_block_id = self
                    .fs()
                    .storage
                    .load_struct::<u32>(
                        Bid::new(self.disk_inode.db_indirect as _),
                        ENTRY_SIZE * (indirect_id / BLK_NENTRY),
                    )
                    .await?;
                assert!(indirect_block_id > 0);
                let device_block_id = device_block_id.to_raw() as u32;
                self.fs()
                    .storage
                    .store_struct::<u32>(
                        Bid::new(indirect_block_id as _),
                        ENTRY_SIZE * (indirect_id as usize % BLK_NENTRY),
                        &device_block_id,
                    )
                    .await?;
                Ok(())
            }
            _ => unimplemented!("triple indirect blocks is not supported"),
        }
    }

    /// Get the indirect blocks
    async fn indirect_blocks(&self) -> Result<Vec<Bid>> {
        let mut indirect_blocks = Vec::new();
        let file_blocks = self.disk_inode.blocks as usize;
        if file_blocks > MAX_NBLOCK_DIRECT {
            assert!(self.disk_inode.indirect > 0);
            indirect_blocks.push(Bid::new(self.disk_inode.indirect as _));
        }
        if file_blocks > MAX_NBLOCK_INDIRECT {
            assert!(self.disk_inode.db_indirect > 0);
            indirect_blocks.push(Bid::new(self.disk_inode.db_indirect as _));
            let indirect_end = (file_blocks - MAX_NBLOCK_INDIRECT) / BLK_NENTRY + 1;
            for i in 0..indirect_end {
                let indirect_id = self
                    .fs()
                    .storage
                    .load_struct::<u32>(Bid::new(self.disk_inode.db_indirect as _), ENTRY_SIZE * i)
                    .await?;
                assert!(indirect_id > 0);
                indirect_blocks.push(Bid::new(indirect_id as _));
            }
        }
        Ok(indirect_blocks)
    }

    async fn get_file_inode_and_entry_id(&self, name: &str) -> Option<(InodeId, FileType, usize)> {
        let name = if name.is_empty() { "." } else { name };
        for i in 0..self.disk_inode.size as usize / DIRENT_SIZE {
            let entry = self.read_direntry(i).await.unwrap();
            if entry.name.as_ref() == name {
                return Some((InodeId::new(entry.id as _), entry.type_.into(), i));
            }
        }
        None
    }

    async fn get_file_inode_id(&self, name: &str) -> Option<InodeId> {
        let name = if name.is_empty() { "." } else { name };
        self.get_file_inode_and_entry_id(name)
            .await
            .map(|(inode_id, _, _)| inode_id)
    }

    /// Init dir content. Insert 2 init entries.
    /// This do not init nlinks, please modify the nlinks in the invoker.
    async fn init_direntry(&mut self, parent: InodeId) -> Result<()> {
        // Insert entries: '.' '..'
        self._resize(DIRENT_SIZE * 2).await?;
        self.write_direntry(
            0,
            &DiskDirEntry {
                id: self.id.to_raw() as u32,
                name: Str256::from("."),
                type_: FileType::Dir as u32,
            },
        )
        .await;
        self.write_direntry(
            1,
            &DiskDirEntry {
                id: parent.to_raw() as u32,
                name: Str256::from(".."),
                type_: FileType::Dir as u32,
            },
        )
        .await;
        Ok(())
    }

    fn direntry_cnt(&self) -> usize {
        self.disk_inode.size as usize / DIRENT_SIZE
    }

    async fn read_direntry(&self, id: usize) -> Result<DiskDirEntry> {
        let mut direntry: DiskDirEntry = unsafe { MaybeUninit::uninit().assume_init() };
        self.read_atomic(DIRENT_SIZE * id, direntry.as_buf_mut())
            .await?;
        Ok(direntry)
    }

    async fn write_direntry(&mut self, id: usize, direntry: &DiskDirEntry) {
        self.write_atomic(DIRENT_SIZE * id, direntry.as_buf())
            .await
            .expect("failed to write dentry");
    }

    async fn append_direntry(&mut self, direntry: &DiskDirEntry) -> Result<()> {
        let size = self.disk_inode.size as usize;
        self._resize(size + DIRENT_SIZE).await?;
        self.write_direntry(self.direntry_cnt() - 1, direntry).await;
        Ok(())
    }

    /// Remove a direntry in middle of file and insert the last one here
    /// WARNING: it may change the index of some entries in dir
    async fn remove_direntry(&mut self, id: usize) -> Result<()> {
        let size = self.disk_inode.size as usize;
        let dirent_count = self.direntry_cnt();
        assert!(id < dirent_count);
        if id < dirent_count - 1 {
            let last_dirent = self.read_direntry(dirent_count - 1).await?;
            self.write_direntry(id, &last_dirent).await;
        }
        self._resize(size - DIRENT_SIZE).await?;
        Ok(())
    }

    /// Resize content size, no matter what type it is
    async fn _resize(&mut self, len: usize) -> Result<()> {
        if len > MAX_FILE_SIZE {
            return_errno!(EINVAL, "size too big");
        }
        let blocks = (len + BLOCK_SIZE - 1) / BLOCK_SIZE;
        assert!(blocks <= MAX_NBLOCK_DOUBLE_INDIRECT);
        use core::cmp::Ordering;
        let old_blocks = self.disk_inode.blocks as usize;
        match blocks.cmp(&old_blocks) {
            Ordering::Equal => {
                if len < self.disk_inode.size as usize {
                    self.write_atomic(len, &ZEROS[..self.disk_inode.size as usize - len])
                        .await
                        .unwrap();
                }
                self.disk_inode.size = len as u32;
            }
            Ordering::Greater => {
                self.alloc_blocks(blocks).await?;
                self.disk_inode.size = len as u32;
            }
            Ordering::Less => {
                self.free_blocks(blocks).await;
                if len > 0 {
                    let len_offset = if len % BLOCK_SIZE == 0 {
                        BLOCK_SIZE
                    } else {
                        len % BLOCK_SIZE
                    };
                    self.write_atomic(len, &ZEROS[len_offset..]).await.unwrap();
                }
                self.disk_inode.size = len as u32;
            }
        }
        Ok(())
    }

    // TODO: Current rollback code is too slow if blocks are not enough to allocate.
    //       Find an efficient way to handle this issue.
    async fn alloc_blocks(&mut self, new_blocks: usize) -> Result<()> {
        // allocate indirect blocks
        self.alloc_indirect_blocks(new_blocks).await?;

        // allocate data blocks
        let old_blocks = self.disk_inode.blocks as usize;
        self.disk_inode.blocks = new_blocks as u32;
        for file_block_id in old_blocks..new_blocks {
            if let Some(device_block_id) = self.fs().alloc_block().await {
                self.set_device_block_id(Bid::new(file_block_id as _), device_block_id)
                    .await
                    .unwrap();
            } else {
                // rollback blocks allocation
                for i in old_blocks..file_block_id {
                    let device_block_id = self.get_device_block_id(Bid::new(i as _)).await.unwrap();
                    self.fs().free_block(device_block_id).await;
                }
                self.free_indirect_blocks(old_blocks).await;
                self.disk_inode.blocks = old_blocks as u32;
                return_errno!(EIO, "no device space");
            }
        }
        Ok(())
    }

    async fn alloc_indirect_blocks(&mut self, new_blocks: usize) -> Result<()> {
        let old_blocks = self.disk_inode.blocks as usize;
        // allocate indirect block if needed
        if old_blocks <= MAX_NBLOCK_DIRECT && new_blocks > MAX_NBLOCK_DIRECT {
            self.disk_inode.indirect = self
                .fs()
                .alloc_block()
                .await
                .ok_or(errno!(EIO, "no device space"))?
                .to_raw() as u32;
        }
        // allocate double indirect block if needed
        if new_blocks > MAX_NBLOCK_INDIRECT {
            if self.disk_inode.db_indirect == 0 {
                if let Some(block_id) = self.fs().alloc_block().await {
                    self.disk_inode.db_indirect = block_id.to_raw() as u32;
                } else {
                    // rollback blocks allocation
                    if old_blocks <= MAX_NBLOCK_DIRECT {
                        self.fs()
                            .free_block(Bid::new(self.disk_inode.indirect as _))
                            .await;
                        self.disk_inode.indirect = 0;
                    }
                    return_errno!(EIO, "no device space");
                }
            }
            let indirect_begin = {
                if old_blocks <= MAX_NBLOCK_INDIRECT {
                    0
                } else {
                    (old_blocks - MAX_NBLOCK_INDIRECT) / BLK_NENTRY + 1
                }
            };
            let indirect_end = (new_blocks - MAX_NBLOCK_INDIRECT) / BLK_NENTRY + 1;
            for i in indirect_begin..indirect_end {
                if let Some(indirect) = self.fs().alloc_block().await {
                    self.fs()
                        .storage
                        .store_struct::<u32>(
                            Bid::new(self.disk_inode.db_indirect as _),
                            ENTRY_SIZE * i,
                            &(indirect.to_raw() as u32),
                        )
                        .await
                        .unwrap();
                } else {
                    // rollback blocks allocation
                    for j in indirect_begin..i {
                        let indirect = self
                            .fs()
                            .storage
                            .load_struct::<u32>(
                                Bid::new(self.disk_inode.db_indirect as _),
                                ENTRY_SIZE * j,
                            )
                            .await
                            .unwrap();
                        self.fs().free_block(Bid::new(indirect as _)).await;
                    }
                    if old_blocks <= MAX_NBLOCK_INDIRECT {
                        self.fs()
                            .free_block(Bid::new(self.disk_inode.db_indirect as _))
                            .await;
                        self.disk_inode.db_indirect = 0;
                    }
                    if old_blocks <= MAX_NBLOCK_DIRECT {
                        self.fs()
                            .free_block(Bid::new(self.disk_inode.indirect as _))
                            .await;
                        self.disk_inode.indirect = 0;
                    }
                    return_errno!(EIO, "no device space");
                }
            }
        }
        Ok(())
    }

    async fn free_blocks(&mut self, new_blocks: usize) {
        // free data blocks
        let old_blocks = self.disk_inode.blocks as usize;
        for file_block_id in new_blocks..old_blocks {
            let device_block_id = self
                .get_device_block_id(Bid::new(file_block_id as _))
                .await
                .unwrap();
            self.fs().free_block(device_block_id).await;
        }

        // free indirect blocks
        self.free_indirect_blocks(new_blocks).await;
        self.disk_inode.blocks = new_blocks as u32;
    }

    async fn free_indirect_blocks(&mut self, new_blocks: usize) {
        let old_blocks = self.disk_inode.blocks as usize;
        // free indirect block if needed
        if new_blocks <= MAX_NBLOCK_DIRECT && old_blocks > MAX_NBLOCK_DIRECT {
            self.fs()
                .free_block(Bid::new(self.disk_inode.indirect as _))
                .await;
            self.disk_inode.indirect = 0;
        }
        // free double indirect block if needed
        if old_blocks > MAX_NBLOCK_INDIRECT {
            let indirect_begin = {
                if new_blocks <= MAX_NBLOCK_INDIRECT {
                    0
                } else {
                    (new_blocks - MAX_NBLOCK_INDIRECT) / BLK_NENTRY + 1
                }
            };
            let indirect_end = (old_blocks - MAX_NBLOCK_INDIRECT) / BLK_NENTRY + 1;
            for i in indirect_begin..indirect_end {
                let indirect = self
                    .fs()
                    .storage
                    .load_struct::<u32>(Bid::new(self.disk_inode.db_indirect as _), ENTRY_SIZE * i)
                    .await
                    .unwrap();
                assert!(indirect > 0);
                self.fs().free_block(Bid::new(indirect as _)).await;
            }
            if new_blocks <= MAX_NBLOCK_INDIRECT {
                assert!(self.disk_inode.db_indirect > 0);
                self.fs()
                    .free_block(Bid::new(self.disk_inode.db_indirect as _))
                    .await;
                self.disk_inode.db_indirect = 0;
            }
        }
    }

    /// Read exact the content one expected.
    /// it is useful to read small data structure such as DirEntry.
    async fn read_atomic(&self, offset: usize, buf: &mut [u8]) -> Result<()> {
        let len = self._read_at(offset, buf).await?;
        if len < buf.len() {
            return_errno!(EIO, "failed to read expected length");
        }
        Ok(())
    }

    /// Write exact the content one expected.
    /// it is useful to read small data structure such as DirEntry.
    async fn write_atomic(&self, offset: usize, buf: &[u8]) -> Result<()> {
        let len = self._write_at(offset, buf).await?;
        if len < buf.len() {
            return_errno!(EIO, "failed to write expected length");
        }
        Ok(())
    }

    /// Read content, no matter what type it is.
    /// Return the length of bytes read, it may be shorter than expected.
    async fn _read_at(&self, offset: usize, buf: &mut [u8]) -> Result<usize> {
        let file_size = self.disk_inode.size as usize;
        let begin = file_size.min(offset);
        let end = file_size.min(offset + buf.len());
        let iter = BlockRangeIter {
            begin,
            end,
            block_size: BLOCK_SIZE,
        };

        const BATCH_READ_THRESHOLD: usize = 2;
        if iter.len() >= BATCH_READ_THRESHOLD {
            return self._read_in_batches(iter, buf).await.map(|_| end - begin);
        }

        self._read_one_by_one(iter, buf).await
    }

    /// Read blocks one by one, allow smaller number of actual read bytes than requested.
    async fn _read_one_by_one(&self, iter: BlockRangeIter, buf: &mut [u8]) -> Result<usize> {
        let mut len_read = 0;
        for range in iter {
            let device_block_id = self.get_device_block_id(range.block_id).await?;
            match self
                .fs()
                .storage
                .read_at(
                    device_block_id,
                    &mut buf[len_read..len_read + range.len()],
                    range.begin,
                )
                .await
            {
                Ok(len) => {
                    len_read += len;
                    if len < range.len() {
                        break;
                    }
                }
                Err(e) if e.errno() == EAGAIN => {
                    break;
                }
                Err(e) => return Err(e),
            }
        }

        Ok(len_read)
    }

    /// Read blocks in consecutive batches, do NOT allow smaller number of actual read bytes than requested.
    async fn _read_in_batches(&self, iter: BlockRangeIter, buf: &mut [u8]) -> Result<()> {
        let sorted_device_blocks = {
            // Collect and sort device blocks' metadata
            let mut device_blocks = Vec::with_capacity(iter.len());
            let mut offset = 0;
            for range in iter {
                device_blocks.push((
                    self.get_device_block_id(range.block_id).await?,
                    offset,
                    range.len(),
                    range.begin,
                ));
                offset += range.len();
            }
            device_blocks.sort_by(|(bid1, _, _, _), (bid2, _, _, _)| bid1.cmp(&bid2));
            device_blocks
        };

        // Group device blocks in consecutive batches
        let device_block_batches = sorted_device_blocks
            .group_by(|(bid1, _, _, _), (bid2, _, _, _)| bid2.to_raw() - bid1.to_raw() == 1);

        // Preform read in batches
        let mut blocks_buf =
            unsafe { Box::new_uninit_slice(sorted_device_blocks.len() * BLOCK_SIZE).assume_init() };
        for device_block_batch in device_block_batches {
            let buf_len = device_block_batch.len() * BLOCK_SIZE;
            let len_read = self
                .fs()
                .storage
                .read_at(
                    device_block_batch.first().unwrap().0,
                    &mut blocks_buf[..buf_len],
                    0,
                )
                .await?;
            if len_read < buf_len {
                // Do not allow partial holed read in a batch read
                return_errno!(EIO, "failed to read expected length in batch read");
            }

            for (nth, (_, offset, len, inner_offset)) in device_block_batch.iter().enumerate() {
                buf[*offset..*offset + len].copy_from_slice(
                    &blocks_buf
                        [nth * BLOCK_SIZE + inner_offset..nth * BLOCK_SIZE + inner_offset + len],
                );
            }
        }

        Ok(())
    }

    /// Write content, no matter what type it is.
    /// Return the length of bytes written, it may be shorter than expected.
    async fn _write_at(&self, offset: usize, buf: &[u8]) -> Result<usize> {
        let file_size = self.disk_inode.size as usize;
        let end_offset = offset + buf.len();
        assert!(file_size >= end_offset);
        let iter = BlockRangeIter {
            begin: offset,
            end: end_offset,
            block_size: BLOCK_SIZE,
        };

        let mut len_written = 0;
        for range in iter {
            let device_block_id = self.get_device_block_id(range.block_id).await?;
            match self
                .fs()
                .storage
                .write_at(
                    device_block_id,
                    &buf[len_written..len_written + range.len()],
                    range.begin,
                )
                .await
            {
                Ok(len) => {
                    len_written += len;
                    if len < range.len() {
                        break;
                    }
                }
                Err(e) if e.errno() == EAGAIN => {
                    break;
                }
                Err(e) => return Err(e),
            }
        }

        Ok(len_written)
    }

    fn inc_nlinks(&mut self) {
        self.disk_inode.nlinks += 1;
    }

    fn dec_nlinks(&mut self) {
        assert!(self.disk_inode.nlinks > 0);
        self.disk_inode.nlinks -= 1;
    }

    fn is_dirty(&self) -> bool {
        self.disk_inode.is_dirty()
    }

    async fn sync_data(&self) -> Result<()> {
        let data_blocks = {
            let mut data_blocks = Vec::new();
            for id in 0..self.disk_inode.blocks {
                let device_block_id = self.get_device_block_id(Bid::new(id as _)).await?;
                data_blocks.push(device_block_id);
            }
            data_blocks
        };
        self.fs().storage.flush_blocks(&data_blocks).await?;
        Ok(())
    }

    async fn sync_metadata(&mut self) -> Result<()> {
        // nlinks is 0, need to delete it
        if self.disk_inode.nlinks == 0 {
            self._resize(0).await?;
            self.disk_inode.sync();
            if !self.is_freed {
                self.fs().free_block(self.id).await;
                self.is_freed = true;
            }
            return Ok(());
        }

        // Write back the metadata
        self.fs()
            .storage
            .store_struct::<DiskInode>(self.id, 0, &self.disk_inode)
            .await?;
        let metadata_blocks = {
            let mut metadata_blocks = self.indirect_blocks().await?;
            metadata_blocks.push(self.id);
            metadata_blocks
        };
        self.fs().storage.flush_blocks(&metadata_blocks).await?;
        self.disk_inode.sync();

        Ok(())
    }
}
