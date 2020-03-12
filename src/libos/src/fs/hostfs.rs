use alloc::string::String;
use alloc::sync::{Arc, Weak};
use core::any::Any;
use rcore_fs::vfs::*;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{SgxMutex as Mutex, SgxMutexGuard as MutexGuard};
use std::untrusted::fs;
use std::untrusted::path::PathEx;

/// Untrusted file system at host
pub struct HostFS {
    path: PathBuf,
    self_ref: Weak<HostFS>,
}

/// INode for `HostFS`
pub struct HNode {
    path: PathBuf,
    file: Mutex<Option<fs::File>>,
    fs: Arc<HostFS>,
}

impl FileSystem for HostFS {
    fn sync(&self) -> Result<()> {
        warn!("HostFS: sync is unimplemented");
        Ok(())
    }

    fn root_inode(&self) -> Arc<dyn INode> {
        Arc::new(HNode {
            path: self.path.clone(),
            file: Mutex::new(None),
            fs: self.self_ref.upgrade().unwrap(),
        })
    }

    fn info(&self) -> FsInfo {
        unimplemented!()
    }
}

impl HostFS {
    /// Create a new `HostFS` from host `path`
    pub fn new(path: impl AsRef<Path>) -> Arc<HostFS> {
        HostFS {
            path: path.as_ref().to_path_buf(),
            self_ref: Weak::default(),
        }
        .wrap()
    }

    /// Wrap pure `HostFS` with Arc
    /// Used in constructors
    fn wrap(self) -> Arc<Self> {
        // Create an Arc, make a Weak from it, then put it into the struct.
        // It's a little tricky.
        let fs = Arc::new(self);
        let weak = Arc::downgrade(&fs);
        let ptr = Arc::into_raw(fs) as *mut Self;
        unsafe {
            (*ptr).self_ref = weak;
        }
        unsafe { Arc::from_raw(ptr) }
    }
}

// workaround for unable to `impl From<std::io::Error> for FsError`
macro_rules! try_std {
    ($ret: expr) => {
        $ret.map_err(|e| e.into_fs_error())?
    };
}

impl INode for HNode {
    fn read_at(&self, offset: usize, buf: &mut [u8]) -> Result<usize> {
        let mut guard = self.open_file()?;
        let file = guard.as_mut().unwrap();
        try_std!(file.seek(SeekFrom::Start(offset as u64)));
        let len = try_std!(file.read(buf));
        Ok(len)
    }

    fn write_at(&self, offset: usize, buf: &[u8]) -> Result<usize> {
        let mut guard = self.open_file()?;
        let file = guard.as_mut().unwrap();
        try_std!(file.seek(SeekFrom::Start(offset as u64)));
        let len = try_std!(file.write(buf));
        Ok(len)
    }

    fn poll(&self) -> Result<PollStatus> {
        unimplemented!()
    }

    fn metadata(&self) -> Result<Metadata> {
        let metadata = try_std!(self.path.metadata());
        Ok(metadata.into_fs_metadata())
    }

    fn set_metadata(&self, metadata: &Metadata) -> Result<()> {
        warn!("HostFS: set_metadata() is unimplemented");
        Ok(())
    }

    fn sync_all(&self) -> Result<()> {
        warn!("HostFS: sync_all() is unimplemented");
        Ok(())
    }

    fn sync_data(&self) -> Result<()> {
        warn!("HostFS: sync_data() is unimplemented");
        Ok(())
    }

    fn resize(&self, len: usize) -> Result<()> {
        warn!("HostFS: resize() is unimplemented");
        Ok(())
    }

    fn create(&self, name: &str, type_: FileType, mode: u32) -> Result<Arc<dyn INode>> {
        let new_path = self.path.join(name);
        if new_path.exists() {
            return Err(FsError::EntryExist);
        }
        match type_ {
            FileType::File => {
                try_std!(fs::File::create(&new_path));
            }
            _ => unimplemented!("only support creating files in HostFS"),
        }
        Ok(Arc::new(HNode {
            path: new_path,
            file: Mutex::new(None),
            fs: self.fs.clone(),
        }))
    }

    fn link(&self, name: &str, other: &Arc<dyn INode>) -> Result<()> {
        let other = other.downcast_ref::<Self>().ok_or(FsError::NotSameFs)?;
        try_std!(fs::hard_link(&other.path, &self.path.join(name)));
        Ok(())
    }

    fn unlink(&self, name: &str) -> Result<()> {
        let new_path = self.path.join(name);
        if new_path.is_file() {
            try_std!(fs::remove_file(new_path));
        } else if new_path.is_dir() {
            unimplemented!("no remove_dir in sgx_std?")
        // fs::remove_dir(new_path)?;
        } else {
            return Err(FsError::EntryNotFound);
        }
        Ok(())
    }

    fn move_(&self, old_name: &str, target: &Arc<dyn INode>, new_name: &str) -> Result<()> {
        let old_path = self.path.join(old_name);
        let new_path = {
            let target = target.downcast_ref::<Self>().ok_or(FsError::NotSameFs)?;
            target.path.join(new_name)
        };
        try_std!(fs::rename(&old_path, &new_path));
        Ok(())
    }

    fn find(&self, name: &str) -> Result<Arc<dyn INode>> {
        let new_path = self.path.join(name);
        if !new_path.exists() {
            return Err(FsError::EntryNotFound);
        }
        Ok(Arc::new(HNode {
            path: new_path,
            file: Mutex::new(None),
            fs: self.fs.clone(),
        }))
    }

    fn get_entry(&self, id: usize) -> Result<String> {
        if !self.path.is_dir() {
            return Err(FsError::NotDir);
        }
        unimplemented!("no read_dir in sgx_std?")
        // FIXME: read_dir

        // self.path
        //     .read_dir()
        //     .map_err(|_| FsError::NotDir)?
        //     .nth(id)
        //     .map_err(|_| FsError::EntryNotFound)?
        //     .file_name()
        //     .into_string()
        //     .map_err(|_| FsError::InvalidParam)
    }

    fn io_control(&self, cmd: u32, data: usize) -> Result<()> {
        warn!("HostFS: io_control is unimplemented");
        Ok(())
    }

    fn fs(&self) -> Arc<dyn FileSystem> {
        self.fs.clone()
    }

    fn as_any_ref(&self) -> &dyn Any {
        self
    }
}

impl HNode {
    /// Ensure to open the file and store a `File` into `self.file`,
    /// return the `MutexGuard`.
    /// If the type of `self.path` is not file, then return Err
    fn open_file(&self) -> Result<MutexGuard<Option<fs::File>>> {
        if !self.path.exists() {
            return Err(FsError::EntryNotFound);
        }
        if !self.path.is_file() {
            return Err(FsError::NotFile);
        }
        let mut maybe_file = self.file.lock().unwrap();
        if maybe_file.is_none() {
            let file = try_std!(fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .open(&self.path));
            *maybe_file = Some(file);
        }
        Ok(maybe_file)
    }
}

trait IntoFsError {
    fn into_fs_error(self) -> FsError;
}

impl IntoFsError for std::io::Error {
    fn into_fs_error(self) -> FsError {
        use std::io::ErrorKind;
        match self.kind() {
            ErrorKind::NotFound => FsError::EntryNotFound,
            ErrorKind::AlreadyExists => FsError::EntryExist,
            ErrorKind::WouldBlock => FsError::Again,
            ErrorKind::InvalidInput => FsError::InvalidParam,
            ErrorKind::InvalidData => FsError::InvalidParam,
            _ => FsError::NotSupported,
        }
    }
}

trait IntoFsMetadata {
    fn into_fs_metadata(self) -> Metadata;
}

impl IntoFsMetadata for fs::Metadata {
    fn into_fs_metadata(self) -> Metadata {
        use sgx_trts::libc;
        use std::os::fs::MetadataExt;
        Metadata {
            dev: self.st_dev() as usize,
            inode: self.st_ino() as usize,
            size: self.st_size() as usize,
            blk_size: self.st_blksize() as usize,
            blocks: self.st_blocks() as usize,
            atime: Timespec {
                sec: self.st_atime(),
                nsec: self.st_atime_nsec() as i32,
            },
            mtime: Timespec {
                sec: self.st_mtime(),
                nsec: self.st_mtime_nsec() as i32,
            },
            ctime: Timespec {
                sec: self.st_ctime(),
                nsec: self.st_ctime_nsec() as i32,
            },
            type_: match self.st_mode() & 0xf000 {
                libc::S_IFCHR => FileType::CharDevice,
                libc::S_IFBLK => FileType::BlockDevice,
                libc::S_IFDIR => FileType::Dir,
                libc::S_IFREG => FileType::File,
                libc::S_IFLNK => FileType::SymLink,
                libc::S_IFSOCK => FileType::Socket,
                _ => unimplemented!("unknown file type"),
            },
            mode: self.st_mode() as u16 & 0o777,
            nlinks: self.st_nlink() as usize,
            uid: self.st_uid() as usize,
            gid: self.st_gid() as usize,
            rdev: self.st_rdev() as usize,
        }
    }
}
