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

impl<T> INode for Dir<T>
where
    T: DirProcINode + Sync + Send + 'static,
{
    fn read_at(&self, offset: usize, buf: &mut [u8]) -> vfs::Result<usize> {
        Err(vfs::FsError::NotFile)
    }

    fn write_at(&self, offset: usize, buf: &[u8]) -> vfs::Result<usize> {
        Err(vfs::FsError::NotFile)
    }

    fn poll(&self) -> vfs::Result<vfs::PollStatus> {
        Err(vfs::FsError::NotFile)
    }

    fn metadata(&self) -> vfs::Result<Metadata> {
        Ok(Metadata {
            dev: 0,
            inode: PROC_INO,
            size: 0,
            blk_size: 0,
            blocks: 0,
            atime: Timespec { sec: 0, nsec: 0 },
            mtime: Timespec { sec: 0, nsec: 0 },
            ctime: Timespec { sec: 0, nsec: 0 },
            type_: vfs::FileType::Dir,
            mode: 0o555,
            nlinks: 1,
            uid: 0,
            gid: 0,
            rdev: 0,
        })
    }

    fn set_metadata(&self, metadata: &Metadata) -> vfs::Result<()> {
        Err(vfs::FsError::PermError)
    }

    fn sync_all(&self) -> vfs::Result<()> {
        Ok(())
    }

    fn sync_data(&self) -> vfs::Result<()> {
        Ok(())
    }

    fn find(&self, name: &str) -> vfs::Result<Arc<dyn INode>> {
        self.inner().find(name)
    }

    fn get_entry(&self, id: usize) -> vfs::Result<String> {
        self.inner().get_entry(id)
    }

    fn iterate_entries(&self, ctx: &mut DirentWriterContext) -> vfs::Result<usize> {
        self.inner().iterate_entries(ctx)
    }

    fn as_any_ref(&self) -> &dyn Any {
        self
    }
}
