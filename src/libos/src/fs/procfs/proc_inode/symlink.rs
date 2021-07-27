use super::*;

pub struct SymLink<T: ProcINode> {
    inner: T,
}

impl<T: ProcINode> SymLink<T> {
    pub fn new(inner: T) -> Self {
        Self { inner }
    }

    pub fn inner(&self) -> &T {
        &self.inner
    }
}

impl<T> INode for SymLink<T>
where
    T: ProcINode + Sync + Send + 'static,
{
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
            type_: vfs::FileType::SymLink,
            mode: 0o777,
            nlinks: 1,
            uid: 0,
            gid: 0,
            rdev: 0,
        })
    }

    impl_inode_for_file_or_symlink!();
}
