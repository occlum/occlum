//! It is infeasible to implement AsyncInode for INode and AsyncFilesystem for FileSystem,
//! because doing upcasting and downcasting between traits is not allowed.
//!
//! The SyncFS and SyncInode are very special structs to wrap any FileSystems and INodes of rcore-fs.
//! It is straightforward to upcast struct to trait or downcast trait to struct, so the sync FileSystem
//! and INode can be easily transformed into async by implementing async-vfs for SyncFS and SyncInode.

use super::*;

use async_io::fs::{Extension, FsMac};
use async_trait::async_trait;
use async_vfs::{AsyncFileSystem, AsyncInode};

/// Fs wrapper for any sync FileSystem
pub struct SyncFS(Arc<dyn FileSystem>);

/// Inode wrapper for any sync INode
pub struct SyncInode(Arc<dyn INode>);

impl SyncFS {
    pub fn new(fs: Arc<dyn FileSystem>) -> Arc<Self> {
        Arc::new(Self(fs))
    }

    pub fn inner(&self) -> &Arc<dyn FileSystem> {
        &self.0
    }
}

impl SyncInode {
    pub fn new(inode: Arc<dyn INode>) -> Arc<Self> {
        Arc::new(Self(inode))
    }

    pub fn inner(&self) -> &Arc<dyn INode> {
        &self.0
    }
}

#[async_trait]
impl AsyncFileSystem for SyncFS {
    async fn sync(&self) -> Result<()> {
        Ok(self.0.sync()?)
    }

    async fn root_inode(&self) -> Arc<dyn AsyncInode> {
        SyncInode::new(self.0.root_inode())
    }

    async fn info(&self) -> FsInfo {
        self.0.info()
    }

    async fn mac(&self) -> FsMac {
        self.0.root_mac()
    }
}

#[async_trait]
impl AsyncInode for SyncInode {
    async fn read_at(&self, offset: usize, buf: &mut [u8]) -> Result<usize> {
        Ok(self.0.read_at(offset, buf)?)
    }

    async fn write_at(&self, offset: usize, buf: &[u8]) -> Result<usize> {
        Ok(self.0.write_at(offset, buf)?)
    }

    async fn metadata(&self) -> Result<Metadata> {
        Ok(self.0.metadata()?)
    }

    async fn set_metadata(&self, metadata: &Metadata) -> Result<()> {
        Ok(self.0.set_metadata(metadata)?)
    }

    async fn sync_all(&self) -> Result<()> {
        Ok(self.0.sync_all()?)
    }

    async fn sync_data(&self) -> Result<()> {
        Ok(self.0.sync_data()?)
    }

    async fn resize(&self, len: usize) -> Result<()> {
        Ok(self.0.resize(len)?)
    }

    async fn fallocate(&self, mode: &FallocateMode, offset: usize, len: usize) -> Result<()> {
        Ok(self.0.fallocate(mode, offset, len)?)
    }

    async fn create(&self, name: &str, type_: FileType, mode: u16) -> Result<Arc<dyn AsyncInode>> {
        Ok(Self::new(self.0.create(name, type_, mode)?))
    }

    async fn link(&self, name: &str, other: &Arc<dyn AsyncInode>) -> Result<()> {
        let other = &other
            .downcast_ref::<Self>()
            .ok_or(errno!(EXDEV, "not same fs"))?;
        Ok(self.0.link(name, other.inner())?)
    }

    async fn unlink(&self, name: &str) -> Result<()> {
        Ok(self.0.unlink(name)?)
    }

    async fn move_(
        &self,
        old_name: &str,
        target: &Arc<dyn AsyncInode>,
        new_name: &str,
    ) -> Result<()> {
        let target = &target
            .downcast_ref::<Self>()
            .ok_or(errno!(EXDEV, "not same fs"))?;
        Ok(self.0.move_(old_name, target.inner(), new_name)?)
    }

    async fn find(&self, name: &str) -> Result<Arc<dyn AsyncInode>> {
        Ok(Self::new(self.0.find(name)?))
    }

    async fn iterate_entries(&self, ctx: &mut DirentWriterContext) -> Result<usize> {
        Ok(self.0.iterate_entries(ctx)?)
    }

    async fn ioctl(&self, cmd: &mut dyn IoctlCmd) -> Result<()> {
        async_io::match_ioctl_cmd_auto_error!(cmd, {
            cmd : NonBuiltinIoctlCmd => {
                    self.0.io_control(cmd.cmd_num().as_u32(), cmd.arg_ptr() as usize)?;
                },
        });
        Ok(())
    }

    fn poll(&self, mask: Events, _poller: Option<&Poller>) -> Events {
        let events = match self.0.poll() {
            Ok(poll_status) => Events::from(poll_status),
            Err(_) => Events::empty(),
        };
        mask & events
    }

    fn ext(&self) -> Option<&Extension> {
        self.0.ext()
    }

    fn as_any_ref(&self) -> &dyn Any {
        self
    }

    fn fs(&self) -> Arc<dyn AsyncFileSystem> {
        SyncFS::new(self.0.fs())
    }
}
