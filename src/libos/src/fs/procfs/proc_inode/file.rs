use super::*;

pub struct File<T: ProcINode> {
    inner: T,
}

impl<T: ProcINode> File<T> {
    pub fn new(inner: T) -> Self {
        Self { inner }
    }

    pub fn inner(&self) -> &T {
        &self.inner
    }
}

#[async_trait]
impl<T> AsyncInode for File<T>
where
    T: ProcINode + Sync + Send + 'static,
{
    async fn read_at(&self, offset: usize, buf: &mut [u8]) -> Result<usize> {
        let data = self.inner().generate_data_in_bytes().await?;
        let start = data.len().min(offset);
        let end = data.len().min(offset + buf.len());
        let len = end - start;
        buf[0..len].copy_from_slice(&data[start..end]);
        Ok(len)
    }

    async fn write_at(&self, _offset: usize, _buf: &[u8]) -> Result<usize> {
        return_errno!(EPERM, "");
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
            type_: FileType::File,
            mode: 0o444,
            nlinks: 1,
            uid: 0,
            gid: 0,
            rdev: 0,
        })
    }

    async fn set_metadata(&self, metadata: &Metadata) -> Result<()> {
        return_errno!(EPERM, "");
    }

    async fn resize(&self, _len: usize) -> Result<()> {
        return_errno!(EPERM, "");
    }

    fn fs(&self) -> Arc<dyn AsyncFileSystem> {
        unimplemented!();
    }

    fn as_any_ref(&self) -> &dyn Any {
        self
    }
}
