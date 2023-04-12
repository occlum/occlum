#![feature(get_mut_unchecked)]
#![feature(new_uninit)]
#![cfg_attr(feature = "sgx", no_std)]

#[cfg(feature = "sgx")]
extern crate sgx_libc as libc;
#[cfg(feature = "sgx")]
//#[macro_use]
extern crate sgx_tstd as std;
#[cfg(feature = "sgx")]
extern crate sgx_types;

//#[macro_use]
extern crate log;

use crate::prelude::*;
use async_io::fs::{
    DirentWriterContext, Extension, FallocateMode, FileType, FsInfo, FsMac, Metadata,
};
use async_io::ioctl::IoctlCmd;
use async_rt::sync::{RwLock as AsyncRwLock, RwLockWriteGuard as AsyncRwLockWriteGuard};
use async_trait::async_trait;
use async_vfs::{AsyncFileSystem, AsyncInode};

use std::any::Any;
use std::{
    collections::HashMap,
    string::String,
    sync::atomic::{AtomicUsize, Ordering},
    sync::{Arc, Weak},
};

mod prelude;
#[cfg(test)]
mod tests;

/// Magic number for unionfs.
pub const UNIONFS_MAGIC: usize = 0x2f8d_be2f;
/// the name of MAC file
const MAC_FILE: &str = ".ufs.mac";
/// the prefix of whiteout file
const WH_PREFIX: &str = ".ufs.wh.";
/// the prefix of opaque file
const OPAQUE_PREFIX: &str = ".ufs.opq.";

/// Union File System
///
/// It allows files and directories of separate file systems, known as branches,
/// to be transparently overlaid, forming a single coherent file system.
pub struct AsyncUnionFS {
    /// Inner file systems
    /// NOTE: the 1st is RW, others are RO
    inners: Vec<Arc<dyn AsyncFileSystem>>,
    /// Weak reference to self
    self_ref: Weak<AsyncUnionFS>,
    /// Root inode
    root_inode: Option<Arc<UnionInode>>,
    /// Allocate inode ID
    next_inode_id: AtomicUsize,
}

impl AsyncUnionFS {
    /// Create a `UnionFS` wrapper for file system `fs`
    pub async fn new(fs: Vec<Arc<dyn AsyncFileSystem>>) -> Result<Arc<Self>> {
        let container_fs = &fs[0];
        match container_fs.root_inode().await.find(MAC_FILE).await {
            Ok(file) => Self::verify_with_mac_file(file, &fs).await?,
            Err(_) => Self::new_mac_file(&fs).await?,
        }

        let mut fs = Arc::new_cyclic(|weak| Self {
            inners: fs,
            self_ref: weak.clone(),
            root_inode: None,
            next_inode_id: AtomicUsize::new(2),
        });
        let root_inode = fs.new_root_inode().await;
        unsafe {
            Arc::get_mut_unchecked(&mut fs).root_inode = Some(root_inode);
        }
        Ok(fs)
    }

    /// Strong type version of `root_inode`
    pub fn root_inode(&self) -> Arc<UnionInode> {
        (*self.root_inode.as_ref().unwrap()).clone()
    }

    /// Verify the MAC(s) in file with the input FS
    async fn verify_with_mac_file(
        mac_file: Arc<dyn AsyncInode>,
        fs: &[Arc<dyn AsyncFileSystem>],
    ) -> Result<()> {
        let mut fs_mac: FsMac = Default::default();
        let mut offset = 0;
        for inner_fs in fs[1..].iter() {
            let len = mac_file.read_at(offset, &mut fs_mac).await?;
            if len != fs_mac.len() {
                return_errno!(EINVAL, "read fs mac failed");
            }
            if inner_fs.mac().await != fs_mac {
                return_errno!(EINVAL, "check fs mac failed");
            }
            offset += len;
        }
        Ok(())
    }

    /// Create a file to record the FS's MAC
    async fn new_mac_file(fs: &[Arc<dyn AsyncFileSystem>]) -> Result<()> {
        let mac_file = fs[0]
            .root_inode()
            .await
            .create(MAC_FILE, FileType::File, 0o777)
            .await?;
        let mut offset = 0;
        for inner_fs in fs[1..].iter() {
            let fs_mac = inner_fs.mac().await;
            let len = mac_file.write_at(offset, &fs_mac).await?;
            if len != fs_mac.len() {
                return_errno!(EINVAL, "write fs mac failed");
            }
            offset += len
        }
        Ok(())
    }

    /// Create a new root inode, only use in the constructor
    async fn new_root_inode(&self) -> Arc<UnionInode> {
        let inodes = {
            let mut inodes = Vec::new();
            for fs in self.inners.iter() {
                let inode = VirtualInode {
                    last_inode: fs.root_inode().await,
                    distance: 0,
                };
                inodes.push(inode);
            }
            inodes
        };

        self.create_inode(
            Weak::default(),
            inodes,
            PathWithMode::new(),
            false,
            None,
            None,
        )
    }

    /// Create a new inode
    fn create_inode(
        &self,
        parent: Weak<UnionInode>,
        inodes: Vec<VirtualInode>,
        path_with_mode: PathWithMode,
        opaque: bool,
        id: Option<usize>,
        ext: Option<Extension>,
    ) -> Arc<UnionInode> {
        Arc::new_cyclic(|weak| UnionInode {
            id: id.unwrap_or_else(|| self.alloc_inode_id()),
            fs: self.self_ref.clone(),
            inner: AsyncRwLock::new(InodeInner {
                inners: inodes,
                cached_children: EntriesMap::new(),
                this: weak.clone(),
                parent: if parent.strong_count() == 0 {
                    weak.clone()
                } else {
                    parent
                },
                path_with_mode,
                opaque,
            }),
            ext: ext.unwrap_or_default(),
        })
    }

    /// Allocate an Inode id
    fn alloc_inode_id(&self) -> usize {
        self.next_inode_id.fetch_add(1, Ordering::SeqCst)
    }
}

#[async_trait]
impl AsyncFileSystem for AsyncUnionFS {
    async fn sync(&self) -> Result<()> {
        for fs in self.inners.iter() {
            fs.sync().await?;
        }
        Ok(())
    }

    async fn root_inode(&self) -> Arc<dyn AsyncInode> {
        self.root_inode()
    }

    async fn info(&self) -> FsInfo {
        let mut merged_info: FsInfo = Default::default();
        for (idx, fs) in self.inners.iter().enumerate() {
            let info = fs.info().await;
            // the writable top layer
            if idx == 0 {
                merged_info.bsize = info.bsize;
                merged_info.frsize = info.frsize;
                merged_info.namemax = info.namemax;
                merged_info.bfree = info.bfree;
                merged_info.bavail = info.bavail;
                merged_info.ffree = info.ffree;
            }
            merged_info.blocks = merged_info.blocks.saturating_add(info.blocks);
            merged_info.files = merged_info.files.saturating_add(info.files);
        }
        merged_info.magic = UNIONFS_MAGIC;
        merged_info
    }
}

/// Inode for `UnionFS`
pub struct UnionInode {
    /// Inode ID
    id: usize,
    /// Reference to FS
    fs: Weak<AsyncUnionFS>,
    /// Inner
    inner: AsyncRwLock<InodeInner>,
    /// Extensions
    ext: Extension,
}

impl UnionInode {
    /// Helper function to create a child inode, if `id` is provided, use it as
    /// the inode id of the new inode, or allocate a new one.
    async fn new_inode(
        fs: &Weak<AsyncUnionFS>,
        parent_guard: &AsyncRwLockWriteGuard<'_, InodeInner>,
        name: &str,
        id: Option<usize>,
        ext: Option<Extension>,
    ) -> Arc<Self> {
        let inodes = {
            let mut inodes = Vec::new();
            for inode in parent_guard.inners.iter() {
                let inode = inode.find(name).await;
                inodes.push(inode);
            }
            inodes
        };
        let mode = inodes
            .iter()
            .find_map(|v| v.as_real())
            .unwrap()
            .metadata()
            .await
            .unwrap()
            .mode;
        let path_with_mode = parent_guard.path_with_mode.with_next(name, mode);
        let opaque = {
            let mut opaque = parent_guard.opaque;
            if let Some(inode) = parent_guard.maybe_container_inode() {
                if inode.find(&name.opaque()).await.is_ok() {
                    opaque = true;
                }
            }
            opaque
        };
        let parent = parent_guard.this.clone();
        let fs = fs.upgrade().unwrap();
        fs.create_inode(parent, inodes, path_with_mode, opaque, id, ext)
    }
}

#[async_trait]
impl AsyncInode for UnionInode {
    async fn read_at(&self, offset: usize, buf: &mut [u8]) -> Result<usize> {
        let inner = self.inner.read().await;
        inner.inode().read_at(offset, buf).await
    }

    async fn write_at(&self, offset: usize, buf: &[u8]) -> Result<usize> {
        let mut inner = self.inner.write().await;
        inner.container_inode().await?.write_at(offset, buf).await
    }

    async fn metadata(&self) -> Result<Metadata> {
        let inner = self.inner.read().await;
        let mut metadata = inner.inode().metadata().await?;
        metadata.inode = self.id;
        Ok(metadata)
    }

    async fn set_metadata(&self, metadata: &Metadata) -> Result<()> {
        let mut inner = self.inner.write().await;
        inner.container_inode().await?.set_metadata(metadata).await
    }

    async fn sync_all(&self) -> Result<()> {
        let inner = self.inner.read().await;
        if let Some(inode) = inner.maybe_container_inode() {
            inode.sync_all().await
        } else {
            Ok(())
        }
    }

    async fn sync_data(&self) -> Result<()> {
        let inner = self.inner.read().await;
        if let Some(inode) = inner.maybe_container_inode() {
            inode.sync_data().await
        } else {
            Ok(())
        }
    }

    async fn fallocate(&self, mode: &FallocateMode, offset: usize, len: usize) -> Result<()> {
        let mut inner = self.inner.write().await;
        inner
            .container_inode()
            .await?
            .fallocate(mode, offset, len)
            .await
    }

    async fn resize(&self, len: usize) -> Result<()> {
        let mut inner = self.inner.write().await;
        inner.container_inode().await?.resize(len).await
    }

    async fn create(&self, name: &str, type_: FileType, mode: u16) -> Result<Arc<dyn AsyncInode>> {
        if self.metadata().await?.type_ != FileType::Dir {
            return_errno!(ENOTDIR, "");
        }
        if name.is_reserved() {
            return_errno!(EINVAL, "invalid name");
        }
        if name.is_self() || name.is_parent() {
            return_errno!(EEXIST, "");
        }
        let mut inner = self.inner.write().await;
        if inner.entries().await.contains_key(name) {
            return_errno!(EEXIST, "");
        }
        let container_inode = inner.container_inode().await?;
        container_inode.create(name, type_, mode).await?;
        if container_inode.find(&name.whiteout()).await.is_ok() {
            match type_ {
                // rename the whiteout file to opaque
                FileType::Dir => {
                    if let Err(e) = container_inode
                        .move_(&name.whiteout(), &container_inode, &name.opaque())
                        .await
                    {
                        // recover
                        container_inode.unlink(name).await?;
                        return Err(e);
                    }
                }
                // unlink the whiteout file
                _ => {
                    if let Err(e) = container_inode.unlink(&name.whiteout()).await {
                        // recover
                        container_inode.unlink(name).await?;
                        return Err(e);
                    }
                }
            }
        }
        let new_inode = Self::new_inode(&self.fs, &inner, name, None, None).await;
        inner
            .entries()
            .await
            .insert(String::from(name), Some(Entry::new(&new_inode).await));
        Ok(new_inode)
    }

    async fn link(&self, name: &str, other: &Arc<dyn AsyncInode>) -> Result<()> {
        if self.metadata().await?.type_ != FileType::Dir {
            return_errno!(ENOTDIR, "");
        }
        if name.is_reserved() {
            return_errno!(EINVAL, "invalid name");
        }
        if name.is_self() || name.is_parent() {
            return_errno!(EEXIST, "");
        }
        if self.inner.write().await.entries().await.contains_key(name) {
            return_errno!(EEXIST, "");
        }
        let child = other
            .downcast_ref::<UnionInode>()
            .ok_or(errno!(EXDEV, "not same fs"))?;
        if child.metadata().await?.type_ == FileType::Dir {
            return_errno!(EISDIR, "");
        }
        // ensure 'child' exists in container
        // copy from image on necessary
        let child_inode = child.inner.write().await.container_inode().await?;
        let mut inner = self.inner.write().await;
        // when we got the lock, the name may have been created by another thread
        if inner.entries().await.contains_key(name) {
            return_errno!(EEXIST, "");
        }
        let this = inner.container_inode().await?;
        this.link(name, &child_inode).await?;
        // unlink the whiteout file
        if let Err(e) = this.unlink(&name.whiteout()).await {
            if e.errno() != ENOENT {
                // recover
                this.unlink(name).await?;
                return Err(e);
            }
        }
        // add `name` to entry cache
        inner.entries().await.insert(String::from(name), None);
        Ok(())
    }

    async fn unlink(&self, name: &str) -> Result<()> {
        if self.metadata().await?.type_ != FileType::Dir {
            return_errno!(ENOTDIR, "");
        }
        if name.is_self() || name.is_parent() {
            return_errno!(EISDIR, "");
        }
        let inode = self.find(name).await?;
        let inode_type = inode.metadata().await?.type_;
        if inode_type == FileType::Dir && inode.list().await?.len() > 2 {
            return_errno!(ENOTEMPTY, "");
        }
        let mut inner = self.inner.write().await;
        // when we got the lock, the entry may have been removed by another thread
        if !inner.entries().await.contains_key(name) {
            return_errno!(ENOENT, "");
        }
        // if file is in container, remove directly
        let dir_inode = inner.container_inode().await?;
        match dir_inode.find(name).await {
            Ok(inode) if inode_type == FileType::Dir => {
                for elem in inode
                    .list()
                    .await?
                    .iter()
                    .filter(|elem| elem.as_str() != "." && elem.as_str() != "..")
                {
                    inode.unlink(elem).await?;
                }
                dir_inode.unlink(name).await?;
                if dir_inode.find(&name.opaque()).await.is_ok() {
                    dir_inode.unlink(&name.opaque()).await?;
                }
            }
            Ok(_) => dir_inode.unlink(name).await?,
            Err(_) => {}
        }
        if inode
            .downcast_ref::<UnionInode>()
            .unwrap()
            .inner
            .read()
            .await
            .has_image_inode()
        {
            // add whiteout to container
            dir_inode
                .create(&name.whiteout(), FileType::File, 0o777)
                .await?;
        }
        // remove `name` from entry cache
        inner.entries().await.remove(name);
        Ok(())
    }

    async fn move_(
        &self,
        old_name: &str,
        target: &Arc<dyn AsyncInode>,
        new_name: &str,
    ) -> Result<()> {
        if old_name.is_self() || old_name.is_parent() {
            return_errno!(EISDIR, "");
        }
        if new_name.is_self() || new_name.is_parent() {
            return_errno!(EISDIR, "");
        }
        if new_name.is_reserved() {
            return_errno!(EINVAL, "invalid name");
        }

        let old = self.find(old_name).await?;
        let old = old.downcast_ref::<UnionInode>().unwrap();
        let old_inode_type = old.metadata().await?.type_;
        // return error when moving a directory from image to container
        // TODO: support the "redirect_dir" feature
        // [Ref](https://www.kernel.org/doc/html/latest/filesystems/overlayfs.html#renaming-directories)
        if old_inode_type == FileType::Dir && old.inner.read().await.has_image_inode() {
            return_errno!(EXDEV, "not same fs");
        }
        let target = target
            .downcast_ref::<UnionInode>()
            .ok_or(errno!(EXDEV, "not same fs"))?;
        if target.metadata().await?.type_ != FileType::Dir {
            return_errno!(ENOTDIR, "");
        }
        // Add the check here to avoid deadlock
        if old.metadata().await?.inode == target.metadata().await?.inode {
            return_errno!(EINVAL, "");
        }

        if let Ok(new_inode) = target.find(new_name).await {
            if old.metadata().await?.inode == new_inode.metadata().await?.inode {
                return Ok(());
            }
            let new_inode_type = new_inode.metadata().await?.type_;
            // if 'old_name' is a directory,
            // 'new_name' must either not exist, or an empty directory.
            match (old_inode_type, new_inode_type) {
                (FileType::Dir, FileType::Dir) => {
                    if new_inode.list().await?.len() > 2 {
                        return_errno!(ENOTEMPTY, "");
                    }
                }
                (FileType::Dir, _) => {
                    return_errno!(ENOTDIR, "");
                }
                (_, FileType::Dir) => {
                    return_errno!(EISDIR, "");
                }
                _ => {}
            }
            target.unlink(new_name).await?;
        }

        // ensure 'old_name' exists in container
        // copy the file from image on necessary
        old.inner.write().await.container_inode().await?;
        // self and target are the same inode
        if self.metadata().await?.inode == target.metadata().await?.inode {
            let mut self_inner = self.inner.write().await;
            let self_inode = self_inner.container_inode().await.unwrap();
            self_inode.move_(old_name, &self_inode, new_name).await?;
            if old.inner.read().await.has_image_inode() {
                if let Err(e) = self_inode
                    .create(&old_name.whiteout(), FileType::File, 0o777)
                    .await
                {
                    // recover
                    self_inode.move_(new_name, &self_inode, old_name).await?;
                    return Err(e);
                }
            }
            if self_inode.find(&new_name.whiteout()).await.is_ok() {
                match old_inode_type {
                    // if is a directory, rename the whiteout to opaque
                    FileType::Dir => {
                        if let Err(e) = self_inode
                            .move_(&new_name.whiteout(), &self_inode, &new_name.opaque())
                            .await
                        {
                            // recover
                            self_inode.move_(new_name, &self_inode, old_name).await?;
                            if old.inner.read().await.has_image_inode() {
                                self_inode.unlink(&old_name.whiteout()).await?;
                            }
                            return Err(e);
                        }
                    }
                    // if is a file, unlink the whiteout file
                    _ => {
                        if let Err(e) = self_inode.unlink(&new_name.whiteout()).await {
                            // recover
                            self_inode.move_(new_name, &self_inode, old_name).await?;
                            if old.inner.read().await.has_image_inode() {
                                self_inode.unlink(&old_name.whiteout()).await?;
                            }
                            return Err(e);
                        }
                    }
                }
            }
            let new_inode = Self::new_inode(
                &self.fs,
                &self_inner,
                new_name,
                Some(old.id),
                Some(old.ext.clone()),
            )
            .await;
            self_inner.entries().await.remove(old_name);
            self_inner
                .entries()
                .await
                .insert(String::from(new_name), Some(Entry::new(&new_inode).await));
        } else {
            // self and target are different inodes
            let (mut self_inner, mut target_inner) = {
                if self.metadata().await?.inode < target.metadata().await?.inode {
                    let self_inner = self.inner.write().await;
                    let target_inner = target.inner.write().await;
                    (self_inner, target_inner)
                } else {
                    let target_inner = target.inner.write().await;
                    let self_inner = self.inner.write().await;
                    (self_inner, target_inner)
                }
            };
            let self_inode = self_inner.container_inode().await.unwrap();
            let target_inode = target_inner.container_inode().await?;
            self_inode.move_(old_name, &target_inode, new_name).await?;
            if old.inner.read().await.has_image_inode() {
                if let Err(e) = self_inode
                    .create(&old_name.whiteout(), FileType::File, 0o777)
                    .await
                {
                    // recover
                    target_inode.move_(new_name, &self_inode, old_name).await?;
                    return Err(e);
                }
            }
            if target_inode.find(&new_name.whiteout()).await.is_ok() {
                match old_inode_type {
                    // if is a directory, rename the whiteout to opaque
                    FileType::Dir => {
                        if let Err(e) = target_inode
                            .move_(&new_name.whiteout(), &target_inode, &new_name.opaque())
                            .await
                        {
                            // recover
                            target_inode.move_(new_name, &self_inode, old_name).await?;
                            if old.inner.read().await.has_image_inode() {
                                self_inode.unlink(&old_name.whiteout()).await?;
                            }
                            return Err(e);
                        }
                    }
                    // if is a file, unlink the whiteout file
                    _ => {
                        if let Err(e) = target_inode.unlink(&new_name.whiteout()).await {
                            // recover
                            target_inode.move_(new_name, &self_inode, old_name).await?;
                            if old.inner.read().await.has_image_inode() {
                                self_inode.unlink(&old_name.whiteout()).await?;
                            }
                            return Err(e);
                        }
                    }
                }
            }
            let new_inode = Self::new_inode(
                &self.fs,
                &target_inner,
                new_name,
                Some(old.id),
                Some(old.ext.clone()),
            )
            .await;
            self_inner.entries().await.remove(old_name);
            target_inner
                .entries()
                .await
                .insert(String::from(new_name), Some(Entry::new(&new_inode).await));
        }
        Ok(())
    }

    async fn find(&self, name: &str) -> Result<Arc<dyn AsyncInode>> {
        if self.metadata().await?.type_ != FileType::Dir {
            return_errno!(ENOTDIR, "");
        }
        let mut inner = self.inner.write().await;

        // Handle the two special entries
        if name.is_self() {
            return Ok(inner.this.upgrade().unwrap());
        } else if name.is_parent() {
            return Ok(inner.parent.upgrade().unwrap());
        }

        let entry_op = inner.entries().await.get(name);
        if entry_op.is_none() {
            return_errno!(ENOENT, "");
        }
        let reused_id = if let Some(entry) = entry_op.unwrap() {
            if let Some(inode) = entry.as_inode() {
                return Ok(inode);
            }
            entry.id()
        } else {
            None
        };
        let new_inode = Self::new_inode(&self.fs, &inner, name, reused_id, None).await;
        inner
            .entries()
            .await
            .insert(String::from(name), Some(Entry::new(&new_inode).await));
        Ok(new_inode)
    }

    async fn iterate_entries(&self, mut ctx: &mut DirentWriterContext) -> Result<usize> {
        if self.metadata().await?.type_ != FileType::Dir {
            return_errno!(ENOTDIR, "");
        }
        let idx = ctx.pos();
        if idx == 0 {
            let this_inode = self.inner.read().await.this.upgrade().unwrap();
            write_inode_entry!(&mut ctx, ".", &this_inode);
        }
        if idx <= 1 {
            let parent_inode = self.inner.read().await.parent.upgrade().unwrap();
            write_inode_entry!(&mut ctx, "..", &parent_inode);
        }

        let mut inner = self.inner.write().await;
        let skipped_children = if idx < 2 { 0 } else { idx - 2 };
        let keys_values: Vec<_> = inner
            .entries()
            .await
            .iter()
            .skip(skipped_children)
            .map(|(name, entry)| (name.clone(), entry.clone()))
            .collect();
        for (name, entry_op) in keys_values.iter() {
            let inode = {
                let (inode_op, reused_id) = if let Some(entry) = entry_op {
                    (entry.as_inode(), entry.id())
                } else {
                    (None, None)
                };
                match inode_op {
                    Some(inode) => inode,
                    None => {
                        let new_inode =
                            Self::new_inode(&self.fs, &inner, name, reused_id, None).await;
                        inner
                            .entries()
                            .await
                            .insert(name.into(), Some(Entry::new(&new_inode).await));
                        new_inode
                    }
                }
            };
            write_inode_entry!(&mut ctx, name, inode);
        }
        Ok(ctx.written_len())
    }

    async fn ioctl(&self, cmd: &mut dyn IoctlCmd) -> Result<()> {
        let inner = self.inner.read().await;
        inner.inode().ioctl(cmd).await
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

/// The mutable part of `UnionInode`
struct InodeInner {
    /// Path from root Inode with mode
    path_with_mode: PathWithMode,
    /// Inodes for each inner file systems
    inners: Vec<VirtualInode>,
    /// Reference to myself
    this: Weak<UnionInode>,
    /// Reference to parent
    parent: Weak<UnionInode>,
    /// Whether uppper directory occludes lower directory
    opaque: bool,
    /// Merged directory entries.
    cached_children: EntriesMap,
}

impl InodeInner {
    /// Merge directory entries from several inodes
    async fn merge_entries(
        inners: &[VirtualInode],
        opaque: bool,
    ) -> Result<HashMap<String, Option<Entry>>> {
        let mut entries = HashMap::new();
        // images
        if !opaque {
            for inode in inners[1..].iter().filter_map(|v| v.as_real()) {
                // if the inode in image FS is not a directory,
                // skip to merge the entries in lower FS
                if inode.metadata().await?.type_ != FileType::Dir {
                    break;
                }
                for name in inode.list().await? {
                    // skip the two special entries
                    if name.is_self() || name.is_parent() {
                        continue;
                    }
                    entries.insert(name, None);
                }
            }
        }
        // container
        if let Some(inode) = inners[0].as_real() {
            for name in inode.list().await? {
                // skip the special entries
                if name.starts_with(OPAQUE_PREFIX)
                    || name == MAC_FILE
                    || name.is_self()
                    || name.is_parent()
                {
                    continue;
                }
                if name.starts_with(WH_PREFIX) {
                    // whiteout
                    entries.remove(name.strip_prefix(WH_PREFIX).unwrap());
                } else {
                    entries.insert(name, None);
                }
            }
        }
        Ok(entries)
    }

    /// Get the merged directory entries, the upper inode must to be a directory
    pub async fn entries(&mut self) -> &mut HashMap<String, Option<Entry>> {
        let cache = &mut self.cached_children.map;
        if !self.cached_children.is_merged {
            let entries = Self::merge_entries(&self.inners, self.opaque)
                .await
                .unwrap();
            //debug!("{:?} cached dirents: {:?}", self.path, entries.keys());
            *cache = entries;
            self.cached_children.is_merged = true;
        }
        cache
    }

    /// Determine the upper inode
    pub fn inode(&self) -> &Arc<dyn AsyncInode> {
        self.inners
            .iter()
            .filter_map(|v| v.as_real())
            .next()
            .unwrap()
    }

    /// Ensure container inode exists in this `UnionInode` and return it.
    ///
    /// If the inode is not exist, first `mkdir -p` the base path.
    /// Then if it is a file, create a copy of the image file;
    /// If it is a directory, create an empty dir.
    /// If it is a symlink, create a copy of the image symlink.
    pub async fn container_inode(&mut self) -> Result<Arc<dyn AsyncInode>> {
        let type_ = self.inode().metadata().await?.type_;
        if type_ != FileType::File
            && type_ != FileType::Dir
            && type_ != FileType::SymLink
            && type_ != FileType::Socket
        {
            return_errno!(EINVAL, "inode type is not supported");
        }
        let VirtualInode {
            mut last_inode,
            distance,
        } = self.inners[0].clone();
        if distance == 0 {
            return Ok(last_inode);
        }

        for (dir_name, mode) in &self.path_with_mode.lastn(distance)[..distance - 1] {
            last_inode = match last_inode.find(dir_name).await {
                Ok(inode) => inode,
                // create dirs to the base path
                Err(e) if e.errno() == ENOENT => {
                    last_inode.create(dir_name, FileType::Dir, *mode).await?
                }
                Err(e) => return Err(e),
            };
        }

        let (last_inode_name, mode) = &self.path_with_mode.lastn(1)[0];
        match last_inode.find(last_inode_name).await {
            Ok(inode) => {
                last_inode = inode;
            }
            Err(e) if e.errno() == ENOENT => {
                // create file/dir/symlink in container
                match type_ {
                    FileType::Dir => {
                        last_inode = last_inode
                            .create(last_inode_name, FileType::Dir, *mode)
                            .await?;
                    }
                    FileType::File => {
                        let last_file_inode = last_inode
                            .create(last_inode_name, FileType::File, *mode)
                            .await?;
                        // copy it from image to container chunk by chunk
                        const BUF_SIZE: usize = 0x10000;
                        let mut buf = unsafe { Box::<[u8; BUF_SIZE]>::new_uninit().assume_init() };
                        let mut offset = 0usize;
                        let mut len = BUF_SIZE;
                        while len == BUF_SIZE {
                            len = match self.inode().read_at(offset, buf.as_mut()).await {
                                Ok(len) => len,
                                Err(e) => {
                                    last_inode.unlink(last_inode_name).await?;
                                    return Err(e);
                                }
                            };
                            match last_file_inode.write_at(offset, &buf[..len]).await {
                                Ok(len_written) if len_written != len => {
                                    last_inode.unlink(last_inode_name).await?;
                                    return_errno!(EIO, "");
                                }
                                Err(e) => {
                                    last_inode.unlink(last_inode_name).await?;
                                    return Err(e);
                                }
                                Ok(_) => {}
                            }
                            offset += len;
                        }
                        last_inode = last_file_inode;
                    }
                    FileType::SymLink | FileType::Socket => {
                        let last_link_inode =
                            last_inode.create(last_inode_name, type_, *mode).await?;
                        let data = match self.inode().read_as_vec().await {
                            Ok(data) => data,
                            Err(e) => {
                                last_inode.unlink(last_inode_name).await?;
                                return Err(e);
                            }
                        };
                        match last_link_inode.write_at(0, &data).await {
                            Ok(len_written) if len_written != data.len() => {
                                last_inode.unlink(last_inode_name).await?;
                                return_errno!(EIO, "");
                            }
                            Err(e) => {
                                last_inode.unlink(last_inode_name).await?;
                                return Err(e);
                            }
                            Ok(_) => {}
                        }
                        last_inode = last_link_inode;
                    }
                    _ => unreachable!(),
                }
            }
            Err(e) => return Err(e),
        }
        self.inners[0] = VirtualInode {
            last_inode: last_inode.clone(),
            distance: 0,
        };
        Ok(last_inode)
    }

    /// Return container inode if it has
    pub fn maybe_container_inode(&self) -> Option<&Arc<dyn AsyncInode>> {
        self.inners[0].as_real()
    }

    /// Whether it has underlying image inodes
    pub fn has_image_inode(&self) -> bool {
        self.inners[1..].iter().any(|v| v.is_real())
    }
}

/// A virtual Inode of a path in a FS
#[derive(Clone)]
struct VirtualInode {
    /// The last valid Inode in the path.
    last_inode: Arc<dyn AsyncInode>,
    /// The distance / depth to the last valid Inode.
    ///
    /// This should be 0 if the last Inode is the current one,
    /// otherwise the path is not exist in the FS, and this is a virtual Inode.
    distance: usize,
}

impl VirtualInode {
    /// Walk this Inode to './name'
    async fn walk(&mut self, name: &str) {
        if self.distance == 0 {
            match self.last_inode.find(name).await {
                Ok(inode) => self.last_inode = inode,
                Err(_) => self.distance = 1,
            }
        } else {
            match name {
                ".." => self.distance -= 1,
                "." => {}
                _ => self.distance += 1,
            }
        }
    }

    /// Find the next Inode at './name'
    pub async fn find(&self, name: &str) -> Self {
        let mut inode = self.clone();
        inode.walk(name).await;
        inode
    }

    /// Whether this is a real Inode
    pub fn is_real(&self) -> bool {
        self.distance == 0
    }

    /// Unwrap the last valid Inode in the path
    pub fn as_real(&self) -> Option<&Arc<dyn AsyncInode>> {
        match self.distance {
            0 => Some(&self.last_inode),
            _ => None,
        }
    }
}

/// Directory entries
struct EntriesMap {
    /// HashMap of the entries
    map: HashMap<String, Option<Entry>>,
    /// Whether the map is merged already
    is_merged: bool,
}

impl EntriesMap {
    fn new() -> Self {
        Self {
            map: HashMap::new(),
            is_merged: false,
        }
    }
}

/// Inode entry. It holds the reference to the real Inode
#[derive(Clone)]
enum Entry {
    /// A Strong reference to the dir inode
    Dir(Arc<UnionInode>),
    /// A weak reference to the not dir inode with id to re-new it if it is dropped
    Other(Weak<UnionInode>, usize),
}

impl Entry {
    async fn new(inode: &Arc<UnionInode>) -> Self {
        if inode.metadata().await.unwrap().type_ == FileType::Dir {
            Self::Dir(Arc::clone(inode))
        } else {
            Self::Other(Arc::downgrade(inode), inode.id)
        }
    }

    fn as_inode(&self) -> Option<Arc<UnionInode>> {
        match self {
            Self::Dir(inode) => Some(Arc::clone(inode)),
            Self::Other(weak_inode, _) => weak_inode.upgrade(),
        }
    }

    fn id(&self) -> Option<usize> {
        match self {
            Self::Other(_, id) => Some(*id),
            _ => None,
        }
    }
}

/// Simple path with access mode
#[derive(Debug, Clone)]
struct PathWithMode(Vec<(String, u16)>);

impl PathWithMode {
    pub fn new() -> Self {
        PathWithMode(Vec::new())
    }

    fn append(&mut self, name: &str, mode: u16) {
        match name {
            "." => {}
            ".." => {
                self.0.pop();
            }
            _ => {
                self.0.push((String::from(name), mode));
            }
        }
    }

    pub fn with_next(&self, name: &str, mode: u16) -> Self {
        let mut next = self.clone();
        next.append(name, mode);
        next
    }

    pub fn lastn(&self, n: usize) -> &[(String, u16)] {
        &self.0[self.0.len() - n..]
    }
}

trait UnionNameExt {
    fn whiteout(&self) -> String;
    fn opaque(&self) -> String;
    fn is_reserved(&self) -> bool;
    fn is_self(&self) -> bool;
    fn is_parent(&self) -> bool;
}

impl UnionNameExt for str {
    fn whiteout(&self) -> String {
        String::from(WH_PREFIX) + self
    }

    fn opaque(&self) -> String {
        String::from(OPAQUE_PREFIX) + self
    }

    fn is_reserved(&self) -> bool {
        self.starts_with(WH_PREFIX) || self.starts_with(OPAQUE_PREFIX) || self == MAC_FILE
    }

    fn is_self(&self) -> bool {
        self == "." || self.is_empty()
    }

    fn is_parent(&self) -> bool {
        self == ".."
    }
}

#[macro_export]
macro_rules! write_inode_entry {
    ($ctx:expr, $name:expr, $inode:expr) => {
        let ctx = $ctx;
        let name = $name;
        let inode = $inode;

        if ctx
            .write_entry(
                name,
                inode.metadata().await?.inode as u64,
                inode.metadata().await?.type_,
            )
            .is_err()
        {
            if ctx.written_len() == 0 {
                return_errno!(EINVAL, "write entry fail");
            } else {
                return Ok(ctx.written_len());
            }
        }
    };
}
