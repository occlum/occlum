use rcore_fs::vfs::{FileSystem, FsError, INode};
use rcore_fs_sefs::SEFS;
use std::fmt;

use super::sgx_impl::SgxStorage;
use super::*;

lazy_static! {
    /// The root of file system
    pub static ref ROOT_INODE: Arc<INode> = {
        let device = Box::new(SgxStorage::new("sefs"));
        let sefs = SEFS::open(device, &time::OcclumTimeProvider)
            .expect("failed to open SEFS");
        sefs.root_inode()
    };
}

pub struct INodeFile {
    inode: Arc<INode>,
    offset: SgxMutex<usize>,
    options: OpenOptions,
}

#[derive(Debug, Clone)]
pub struct OpenOptions {
    pub read: bool,
    pub write: bool,
    /// Before each write, the file offset is positioned at the end of the file.
    pub append: bool,
}

impl File for INodeFile {
    fn read(&self, buf: &mut [u8]) -> Result<usize, Error> {
        if !self.options.read {
            return errno!(EBADF, "File not readable");
        }
        let mut offset = self.offset.lock().unwrap();
        let len = self.inode.read_at(*offset, buf)?;
        *offset += len;
        Ok(len)
    }

    fn write(&self, buf: &[u8]) -> Result<usize, Error> {
        if !self.options.write {
            return errno!(EBADF, "File not writable");
        }
        let mut offset = self.offset.lock().unwrap();
        if self.options.append {
            let info = self.inode.metadata()?;
            *offset = info.size;
        }
        let len = self.inode.write_at(*offset, buf)?;
        *offset += len;
        Ok(len)
    }

    fn read_at(&self, offset: usize, buf: &mut [u8]) -> Result<usize, Error> {
        if !self.options.read {
            return errno!(EBADF, "File not readable");
        }
        let len = self.inode.read_at(offset, buf)?;
        Ok(len)
    }

    fn write_at(&self, offset: usize, buf: &[u8]) -> Result<usize, Error> {
        if !self.options.write {
            return errno!(EBADF, "File not writable");
        }
        let len = self.inode.write_at(offset, buf)?;
        Ok(len)
    }

    fn readv(&self, bufs: &mut [&mut [u8]]) -> Result<usize, Error> {
        if !self.options.read {
            return errno!(EBADF, "File not readable");
        }
        let mut offset = self.offset.lock().unwrap();
        let mut total_len = 0;
        for buf in bufs {
            match self.inode.read_at(*offset, buf) {
                Ok(len) => {
                    total_len += len;
                    *offset += len;
                }
                Err(_) if total_len != 0 => break,
                Err(e) => return Err(e.into()),
            }
        }
        Ok(total_len)
    }

    fn writev(&self, bufs: &[&[u8]]) -> Result<usize, Error> {
        if !self.options.write {
            return errno!(EBADF, "File not writable");
        }
        let mut offset = self.offset.lock().unwrap();
        if self.options.append {
            let info = self.inode.metadata()?;
            *offset = info.size;
        }
        let mut total_len = 0;
        for buf in bufs {
            match self.inode.write_at(*offset, buf) {
                Ok(len) => {
                    total_len += len;
                    *offset += len;
                }
                Err(_) if total_len != 0 => break,
                Err(e) => return Err(e.into()),
            }
        }
        Ok(total_len)
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

    fn metadata(&self) -> Result<Metadata, Error> {
        let metadata = self.inode.metadata()?;
        Ok(metadata)
    }

    fn set_len(&self, len: u64) -> Result<(), Error> {
        if !self.options.write {
            return errno!(EBADF, "File not writable. Can't set len.");
        }
        self.inode.resize(len as usize)?;
        Ok(())
    }

    fn sync_all(&self) -> Result<(), Error> {
        self.inode.sync_all()?;
        Ok(())
    }

    fn sync_data(&self) -> Result<(), Error> {
        self.inode.sync_data()?;
        Ok(())
    }

    fn read_entry(&self) -> Result<String, Error> {
        if !self.options.read {
            return errno!(EBADF, "File not readable. Can't read entry.");
        }
        let mut offset = self.offset.lock().unwrap();
        let name = self.inode.get_entry(*offset)?;
        *offset += 1;
        Ok(name)
    }

    fn as_any(&self) -> &Any {
        self
    }
}

impl INodeFile {
    pub fn open(inode: Arc<INode>, options: OpenOptions) -> Result<Self, Error> {
        Ok(INodeFile {
            inode,
            offset: SgxMutex::new(0),
            options,
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
        write!(
            f,
            "INodeFile {{ inode: ???, pos: {}, options: {:?} }}",
            *self.offset.lock().unwrap(),
            self.options
        )
    }
}

pub trait INodeExt {
    fn read_as_vec(&self) -> Result<Vec<u8>, Error>;
}

impl INodeExt for INode {
    fn read_as_vec(&self) -> Result<Vec<u8>, Error> {
        let size = self.metadata()?.size;
        let mut buf = Vec::with_capacity(size);
        unsafe {
            buf.set_len(size);
        }
        self.read_at(0, buf.as_mut_slice())?;
        Ok(buf)
    }
}
