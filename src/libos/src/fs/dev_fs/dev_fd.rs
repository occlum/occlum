use super::*;
pub struct DevFd;

// Implement /dev/fd as a symlink to /proc/self/fd
impl INode for DevFd {
    fn read_at(&self, offset: usize, buf: &mut [u8]) -> vfs::Result<usize> {
        let proc_fd = "/proc/self/fd";
        for (tgt, src) in buf.iter_mut().zip(proc_fd.as_bytes().iter()) {
            *tgt = *src;
        }
        Ok(proc_fd.len())
    }

    fn write_at(&self, offset: usize, buf: &[u8]) -> vfs::Result<usize> {
        Err(vfs::FsError::PermError)
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
            type_: vfs::FileType::SymLink,
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
