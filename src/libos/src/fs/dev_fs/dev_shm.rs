use super::*;

#[derive(Debug)]
pub struct DevShm;

impl INode for DevShm {
    fn read_at(&self, offset: usize, buf: &mut [u8]) -> vfs::Result<usize> {
        Err(vfs::FsError::NotFile)
    }

    fn write_at(&self, offset: usize, buf: &[u8]) -> vfs::Result<usize> {
        Err(vfs::FsError::NotFile)
    }

    fn metadata(&self) -> vfs::Result<Metadata> {
        Ok(Metadata {
            dev: 0,
            inode: 2,
            size: 0,
            blk_size: 0,
            blocks: 0,
            atime: Timespec { sec: 0, nsec: 0 },
            mtime: Timespec { sec: 0, nsec: 0 },
            ctime: Timespec { sec: 0, nsec: 0 },
            type_: vfs::FileType::Dir,
            mode: 0o777,
            nlinks: 1,
            uid: 0,
            gid: 0,
            rdev: 0,
        })
    }

    fn as_any_ref(&self) -> &dyn Any {
        self
    }
}
