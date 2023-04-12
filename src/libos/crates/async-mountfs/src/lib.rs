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
use async_io::event::{Events, Poller};
use async_io::fs::{
    DirentWriterContext, Extension, FallocateMode, FileType, FsInfo, FsMac, Metadata,
};
use async_io::ioctl::IoctlCmd;
use async_rt::sync::RwLock as AsyncRwLock;
use async_trait::async_trait;
use async_vfs::{AsyncFileSystem, AsyncInode};

use std::any::Any;
use std::{
    collections::HashMap,
    string::String,
    sync::{Arc, Weak},
};

mod prelude;
#[cfg(test)]
mod tests;

/// The filesystem on which all the other filesystems are mounted
pub struct AsyncMountFS {
    /// The inner file system
    inner: Arc<dyn AsyncFileSystem>,
    /// All mounted children file systems
    mountpoints: AsyncRwLock<HashMap<InodeId, Arc<Self>>>,
    /// The mount point of this file system
    self_mountpoint: Option<Arc<AsyncMInode>>,
    /// Weak reference to self
    self_ref: Weak<Self>,
}

type InodeId = usize;

impl AsyncMountFS {
    /// Create an `AsyncMountFS` wrapper for the file system `fs`
    pub fn new(fs: Arc<dyn AsyncFileSystem>) -> Arc<Self> {
        Self {
            inner: fs,
            mountpoints: AsyncRwLock::new(HashMap::new()),
            self_mountpoint: None,
            self_ref: Weak::default(),
        }
        .wrap()
    }

    fn wrap(self) -> Arc<Self> {
        let fs = Arc::new(self);
        let weak = Arc::downgrade(&fs);
        let ptr = Arc::into_raw(fs) as *mut Self;
        unsafe {
            (*ptr).self_ref = weak;
            Arc::from_raw(ptr)
        }
    }

    async fn mountpoint_root_inode(&self) -> Arc<AsyncMInode> {
        AsyncMInode::new(
            self.inner.root_inode().await,
            self.self_ref.upgrade().unwrap(),
        )
    }
}

#[async_trait]
impl AsyncFileSystem for AsyncMountFS {
    async fn sync(&self) -> Result<()> {
        self.inner.sync().await?;
        for mount_fs in self.mountpoints.read().await.values() {
            mount_fs.sync().await?;
        }
        Ok(())
    }

    async fn root_inode(&self) -> Arc<dyn AsyncInode> {
        match &self.self_mountpoint {
            Some(inode) => inode.vfs.root_inode().await,
            None => self.mountpoint_root_inode().await,
        }
    }

    async fn info(&self) -> FsInfo {
        self.inner.info().await
    }

    async fn mac(&self) -> FsMac {
        self.inner.mac().await
    }
}

/// Inode for `AsyncMountFS`
pub struct AsyncMInode {
    /// The inner Inode
    inner: Arc<dyn AsyncInode>,
    /// Associated `AsyncMountFS`
    vfs: Arc<AsyncMountFS>,
    /// Weak reference to self
    self_ref: Weak<AsyncMInode>,
}

impl AsyncMInode {
    pub fn new(inode: Arc<dyn AsyncInode>, vfs: Arc<AsyncMountFS>) -> Arc<Self> {
        Self {
            inner: inode,
            vfs,
            self_ref: Weak::default(),
        }
        .wrap()
    }

    fn wrap(self) -> Arc<Self> {
        let inode = Arc::new(self);
        let weak = Arc::downgrade(&inode);
        let ptr = Arc::into_raw(inode) as *mut Self;
        unsafe {
            (*ptr).self_ref = weak;
            Arc::from_raw(ptr)
        }
    }

    pub fn inner(&self) -> &Arc<dyn AsyncInode> {
        &self.inner
    }

    /// Get the root inode of the mounted fs at here.
    /// Return self if no mounted fs.
    async fn overlaid_inode(&self) -> Arc<AsyncMInode> {
        let inode_id = self.metadata().await.unwrap().inode;
        if let Some(sub_vfs) = self.vfs.mountpoints.read().await.get(&inode_id) {
            sub_vfs.mountpoint_root_inode().await
        } else {
            self.self_ref.upgrade().unwrap()
        }
    }

    /// If is the root inode of its fs
    async fn is_mountpoint_root(&self) -> bool {
        self.inner
            .fs()
            .root_inode()
            .await
            .metadata()
            .await
            .unwrap()
            .inode
            == self.inner.metadata().await.unwrap().inode
    }

    /// Strong type version of `create()`
    pub async fn create(&self, name: &str, type_: FileType, mode: u16) -> Result<Arc<Self>> {
        let inode = Self::new(
            self.inner.create(name, type_, mode).await?,
            self.vfs.clone(),
        );
        Ok(inode)
    }

    /// Strong type version of `find()`
    pub async fn find(&self, name: &str) -> Result<Arc<Self>> {
        // Self arc may change if going up fs border
        let mut this = self.self_ref.upgrade().unwrap();

        let inode = loop {
            match name {
                "" | "." => break this,
                ".." => {
                    // Going Up
                    if this.is_mountpoint_root().await {
                        // May trespass filesystem border
                        match &this.vfs.self_mountpoint {
                            Some(inode) => {
                                this = inode.clone();
                            }
                            None => break this,
                        }
                    } else {
                        // Not trespass filesystem border, in same filesystem
                        break Self::new(this.inner.find("..").await?, this.vfs.clone());
                    }
                }
                _ => {
                    // Going down
                    // May trespass filesystem border
                    break Self::new(
                        this.overlaid_inode().await.inner.find(name).await?,
                        this.vfs.clone(),
                    )
                    .overlaid_inode()
                    .await;
                }
            }
        };

        Ok(inode)
    }
}

#[async_trait]
impl AsyncInode for AsyncMInode {
    async fn read_at(&self, offset: usize, buf: &mut [u8]) -> Result<usize> {
        self.inner.read_at(offset, buf).await
    }

    async fn write_at(&self, offset: usize, buf: &[u8]) -> Result<usize> {
        self.inner.write_at(offset, buf).await
    }

    async fn metadata(&self) -> Result<Metadata> {
        self.inner.metadata().await
    }

    async fn set_metadata(&self, metadata: &Metadata) -> Result<()> {
        self.inner.set_metadata(metadata).await
    }

    async fn sync_all(&self) -> Result<()> {
        self.inner.sync_all().await
    }

    async fn sync_data(&self) -> Result<()> {
        self.inner.sync_data().await
    }

    async fn resize(&self, len: usize) -> Result<()> {
        self.inner.resize(len).await
    }

    async fn fallocate(&self, mode: &FallocateMode, offset: usize, len: usize) -> Result<()> {
        self.inner.fallocate(mode, offset, len).await
    }

    async fn create(&self, name: &str, type_: FileType, mode: u16) -> Result<Arc<dyn AsyncInode>> {
        Ok(self.create(name, type_, mode).await?)
    }

    async fn link(&self, name: &str, other: &Arc<dyn AsyncInode>) -> Result<()> {
        let other = &other
            .downcast_ref::<Self>()
            .ok_or(errno!(EXDEV, "not same fs"))?
            .inner;
        self.inner.link(name, other).await
    }

    async fn unlink(&self, name: &str) -> Result<()> {
        let inode_id = self.inner.find(name).await?.metadata().await?.inode;
        if self.vfs.mountpoints.read().await.contains_key(&inode_id) {
            return_errno!(EBUSY, "inode is being mounted");
        }
        self.inner.unlink(name).await
    }

    async fn move_(
        &self,
        old_name: &str,
        target: &Arc<dyn AsyncInode>,
        new_name: &str,
    ) -> Result<()> {
        let target = &target
            .downcast_ref::<Self>()
            .ok_or(errno!(EXDEV, "not same fs"))?
            .inner;
        self.inner.move_(old_name, target, new_name).await
    }

    async fn find(&self, name: &str) -> Result<Arc<dyn AsyncInode>> {
        Ok(self.find(name).await?)
    }

    async fn iterate_entries(&self, ctx: &mut DirentWriterContext) -> Result<usize> {
        self.inner.iterate_entries(ctx).await
    }

    async fn ioctl(&self, cmd: &mut dyn IoctlCmd) -> Result<()> {
        self.inner.ioctl(cmd).await
    }

    async fn mount(&self, fs: Arc<dyn AsyncFileSystem>) -> Result<()> {
        let metadata = self.inner.metadata().await?;
        if metadata.type_ != FileType::Dir {
            return_errno!(ENOTDIR, "self is not dir");
        }
        let new_fs = AsyncMountFS {
            inner: fs,
            mountpoints: AsyncRwLock::new(HashMap::new()),
            self_mountpoint: Some(self.self_ref.upgrade().unwrap()),
            self_ref: Weak::default(),
        }
        .wrap();
        self.vfs
            .mountpoints
            .write()
            .await
            .insert(metadata.inode, new_fs);
        Ok(())
    }

    async fn umount(&self) -> Result<()> {
        let metadata = self.inner.metadata().await?;
        if metadata.type_ != FileType::Dir {
            return_errno!(ENOTDIR, "self is not dir");
        }
        if !self.is_mountpoint_root().await {
            return_errno!(EINVAL, "self is not mount point");
        }

        // Here it is the mountpoint
        match &self.vfs.self_mountpoint {
            Some(inode) => {
                self.vfs.sync().await?;
                let inode_id = inode.metadata().await.unwrap().inode;
                inode.vfs.mountpoints.write().await.remove(&inode_id);
            }
            None => {
                return_errno!(EPERM, "cannot umount rootfs");
            }
        }
        Ok(())
    }

    fn poll(&self, mask: Events, poller: Option<&Poller>) -> Events {
        self.inner.poll(mask, poller)
    }

    fn fs(&self) -> Arc<dyn AsyncFileSystem> {
        self.vfs.clone()
    }

    fn ext(&self) -> Option<&Extension> {
        self.inner.ext()
    }

    fn as_any_ref(&self) -> &dyn Any {
        self
    }
}
