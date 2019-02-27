use super::*;

use std::fmt;
use rcore_fs::vfs::{INode, FileSystem, FsError};
use rcore_fs_sefs::{SEFS, dev::sgx_impl::{SgxStorage, SgxTimeProvider}};

lazy_static! {
    /// The root of file system
    pub static ref ROOT_INODE: Arc<INode> = {
        let device = Box::new(SgxStorage::new("sefs"));
        let sefs = SEFS::open(device, &SgxTimeProvider).expect("failed to open SEFS");
        sefs.root_inode()
    };
}

pub struct INodeFile {
    inode: Arc<INode>,
    offset: SgxMutex<usize>,
    is_readable: bool,
    is_writable: bool,
    is_append: bool,
}

impl File for INodeFile {
    fn read(&self, buf: &mut [u8]) -> Result<usize, Error> {
        if !self.is_readable {
            return Err(Error::new(Errno::EBADF, "File not readable"));
        }
        let mut offset = self.offset.lock().unwrap();
        let len = self.inode.read_at(*offset, buf)?;
        *offset += len;
        Ok(len)
    }

    fn write(&self, buf: &[u8]) -> Result<usize, Error> {
        if !self.is_writable {
            return Err(Error::new(Errno::EBADF, "File not writable"));
        }
        let mut offset = self.offset.lock().unwrap();
        if self.is_append {
            let info = self.inode.metadata()?;
            *offset = info.size;
        }
        let len = self.inode.write_at(*offset, buf)?;
        *offset += len;
        Ok(len)
    }

    fn readv(&self, bufs: &mut [&mut [u8]]) -> Result<usize, Error> {
        Err(Error::new(Errno::ENOSYS, "Not implemented"))
    }

    fn writev(&self, bufs: &[&[u8]]) -> Result<usize, Error> {
        Err(Error::new(Errno::ENOSYS, "Not implemented"))
    }

    fn seek(&self, pos: SeekFrom) -> Result<off_t, Error> {
        let mut offset = self.offset.lock().unwrap();
        *offset = match pos {
            SeekFrom::Start(off) => off as usize,
            SeekFrom::End(off) => (self.inode.metadata()?.size as i64 + off) as usize,
            SeekFrom::Current(off) => (*offset as i64 + off) as usize,
        };
        Ok(*offset as i64)
    }
}

impl INodeFile {
    pub fn open(path: &str, is_readable: bool, is_writable: bool, is_append: bool) -> Result<Self, Error> {
        Ok(INodeFile {
            inode: ROOT_INODE.lookup(path)?,
            offset: SgxMutex::new(0),
            is_readable,
            is_writable,
            is_append,
        })
    }
}

/// Convert VFS Error to libc error code
impl From<FsError> for Error {
    fn from(error: FsError) -> Self {
        let errno = match error {
            FsError::NotSupported => ENOSYS,
            FsError::NotFile => EISDIR,
            FsError::IsDir => EISDIR,
            FsError::NotDir => ENOTDIR,
            FsError::EntryNotFound => ENOENT,
            FsError::EntryExist => EEXIST,
            FsError::NotSameFs => EXDEV,
            FsError::InvalidParam => EINVAL,
            FsError::NoDeviceSpace => ENOMEM,
            FsError::DirRemoved => ENOENT,
            FsError::DirNotEmpty => ENOTEMPTY,
            FsError::WrongFs => EINVAL,
            FsError::DeviceError => EIO,
        };
        Error::new(errno, "")
    }
}

impl Debug for INodeFile {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "INodeFile {{ pos: {}, inode: ??? }}", *self.offset.lock().unwrap())
    }
}
