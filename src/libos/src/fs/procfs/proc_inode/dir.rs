use super::*;

pub struct Dir<T: DirProcINode> {
    inner: T,
}

impl<T: DirProcINode> Dir<T> {
    pub fn new(inner: T) -> Self {
        Self { inner }
    }

    pub fn inner(&self) -> &T {
        &self.inner
    }
}

#[async_trait]
impl<T> AsyncInode for Dir<T>
where
    T: DirProcINode + Sync + Send + 'static,
{
    async fn read_at(&self, _offset: usize, _buf: &mut [u8]) -> Result<usize> {
        return_errno!(EISDIR, "not file");
    }

    async fn write_at(&self, _offset: usize, _buf: &[u8]) -> Result<usize> {
        return_errno!(EISDIR, "not file");
    }

    async fn metadata(&self) -> Result<Metadata> {
        Ok(Metadata {
            dev: 0,
            inode: PROC_INO,
            size: 0,
            blk_size: 0,
            blocks: 0,
            atime: Timespec { sec: 0, nsec: 0 },
            mtime: Timespec { sec: 0, nsec: 0 },
            ctime: Timespec { sec: 0, nsec: 0 },
            type_: FileType::Dir,
            mode: 0o555,
            nlinks: 1,
            uid: 0,
            gid: 0,
            rdev: 0,
        })
    }

    async fn set_metadata(&self, _metadata: &Metadata) -> Result<()> {
        return_errno!(EPERM, "");
    }

    async fn create(
        &self,
        _name: &str,
        _type_: FileType,
        _mode: u16,
    ) -> Result<Arc<dyn AsyncInode>> {
        return_errno!(EPERM, "");
    }

    async fn link(&self, _name: &str, _other: &Arc<dyn AsyncInode>) -> Result<()> {
        return_errno!(EPERM, "");
    }

    async fn unlink(&self, _name: &str) -> Result<()> {
        return_errno!(EPERM, "");
    }

    async fn move_(
        &self,
        _old_name: &str,
        _target: &Arc<dyn AsyncInode>,
        _new_name: &str,
    ) -> Result<()> {
        return_errno!(EPERM, "");
    }

    async fn find(&self, name: &str) -> Result<Arc<dyn AsyncInode>> {
        self.inner().find(name).await
    }

    async fn iterate_entries(&self, ctx: &mut DirentWriterContext) -> Result<usize> {
        self.inner().iterate_entries(ctx).await
    }

    fn fs(&self) -> Arc<dyn AsyncFileSystem> {
        unimplemented!();
    }

    fn as_any_ref(&self) -> &dyn Any {
        self
    }
}
