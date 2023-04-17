use super::*;

#[derive(Debug)]
pub struct DevZero;

impl INode for DevZero {
    fn read_at(&self, offset: usize, buf: &mut [u8]) -> vfs::Result<usize> {
        for b in buf.iter_mut() {
            *b = 0;
        }
        Ok(buf.len())
    }

    fn write_at(&self, offset: usize, buf: &[u8]) -> vfs::Result<usize> {
        Ok(buf.len())
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
            mode: 0o666,
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
