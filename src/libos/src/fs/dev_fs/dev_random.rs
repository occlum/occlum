use super::*;
use crate::misc;

#[derive(Debug)]
pub struct DevRandom;

impl INode for DevRandom {
    fn read_at(&self, offset: usize, buf: &mut [u8]) -> vfs::Result<usize> {
        misc::get_random(buf).map_err(|_| FsError::Again)?;
        Ok(buf.len())
    }

    fn write_at(&self, offset: usize, buf: &[u8]) -> vfs::Result<usize> {
        Err(FsError::PermError)
    }

    fn poll(&self) -> vfs::Result<vfs::PollStatus> {
        Ok(vfs::PollStatus {
            read: true,
            write: false,
            error: false,
        })
    }

    fn metadata(&self) -> vfs::Result<Metadata> {
        Ok(Metadata {
            dev: 1,
            inode: 0,
            size: 0,
            blk_size: 0,
            blocks: 0,
            atime: Timespec { sec: 0, nsec: 0 },
            mtime: Timespec { sec: 0, nsec: 0 },
            ctime: Timespec { sec: 0, nsec: 0 },
            type_: vfs::FileType::CharDevice,
            mode: 0o444,
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
