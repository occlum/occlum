use crate::fs::fs_ops::fetch_host_statfs;
use alloc::string::String;
use alloc::sync::{Arc, Weak};
use core::any::Any;
use rcore_fs::vfs::*;
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::unix::fs::{DirEntryExt, FileExt, FileTypeExt, PermissionsExt};
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
    type_: FileType,
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
            type_: FileType::Dir,
            fs: self.self_ref.upgrade().unwrap(),
        })
    }

    fn info(&self) -> FsInfo {
        let statfs = fetch_host_statfs(&self.path.to_string_lossy()).unwrap();
        statfs.into()
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
        if !self.is_file() {
            return Err(FsError::NotFile);
        }
        let mut guard = self.open_file()?;
        let file = guard.as_mut().unwrap();
        let len = try_std!(file.read_at(buf, offset as u64));
        Ok(len)
    }

    fn write_at(&self, offset: usize, buf: &[u8]) -> Result<usize> {
        if !self.is_file() {
            return Err(FsError::NotFile);
        }
        let mut guard = self.open_file()?;
        let file = guard.as_mut().unwrap();
        let len = try_std!(file.write_at(buf, offset as u64));
        Ok(len)
    }

    fn metadata(&self) -> Result<Metadata> {
        let metadata = if self.is_file() {
            let guard = self.open_file()?;
            let file = guard.as_ref().unwrap();
            try_std!(file.metadata())
        } else {
            try_std!(self.path.metadata())
        };
        Ok(metadata.into_fs_metadata())
    }

    fn set_metadata(&self, metadata: &Metadata) -> Result<()> {
        warn!(
            "HostFS: set_metadata() only support chmod: {:#o}",
            metadata.mode
        );
        let perms = fs::Permissions::from_mode(metadata.mode as u32);
        try_std!(fs::set_permissions(&self.path, perms));
        Ok(())
    }

    fn sync_all(&self) -> Result<()> {
        if self.is_file() {
            let guard = self.open_file()?;
            let file = guard.as_ref().unwrap();
            try_std!(file.sync_all());
        } else {
            warn!("no sync_all method about dir, do nothing");
        }
        Ok(())
    }

    fn sync_data(&self) -> Result<()> {
        if self.is_file() {
            let guard = self.open_file()?;
            let file = guard.as_ref().unwrap();
            try_std!(file.sync_data());
        } else {
            warn!("no sync_data method about dir, do nothing");
        }
        Ok(())
    }

    fn resize(&self, len: usize) -> Result<()> {
        if !self.is_file() {
            return Err(FsError::NotFile);
        }
        let guard = self.open_file()?;
        let file = guard.as_ref().unwrap();
        try_std!(file.set_len(len as u64));
        Ok(())
    }

    fn create(&self, name: &str, type_: FileType, mode: u16) -> Result<Arc<dyn INode>> {
        let new_path = self.path.join(name);
        if new_path.exists() {
            return Err(FsError::EntryExist);
        }
        let perms = fs::Permissions::from_mode(mode as u32);
        let file = match type_ {
            FileType::File => {
                let file = try_std!(fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .open(&new_path));
                try_std!(file.set_permissions(perms));
                Some(file)
            }
            FileType::Dir => {
                try_std!(fs::create_dir(&new_path));
                try_std!(fs::set_permissions(&new_path, perms));
                None
            }
            _ => {
                warn!("only support creating regular file or directory in HostFS");
                return Err(FsError::PermError);
            }
        };

        Ok(Arc::new(HNode {
            path: new_path,
            file: Mutex::new(file),
            type_,
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
            try_std!(fs::remove_dir(new_path));
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
        let metadata = fs::metadata(&new_path).map_err(|_| FsError::EntryNotFound)?;
        let file = if metadata.is_file() {
            try_std!(fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(&new_path)
                .map(Some))
        } else {
            None
        };

        Ok(Arc::new(HNode {
            path: new_path,
            file: Mutex::new(file),
            type_: metadata.file_type().into_fs_filetype(),
            fs: self.fs.clone(),
        }))
    }

    fn get_entry(&self, id: usize) -> Result<String> {
        if !self.is_dir() {
            return Err(FsError::NotDir);
        }
        if let Some(entry) = try_std!(self.path.read_dir()).nth(id) {
            try_std!(entry)
                .file_name()
                .into_string()
                .map_err(|_| FsError::InvalidParam)
        } else {
            return Err(FsError::EntryNotFound);
        }
    }

    fn iterate_entries(&self, ctx: &mut DirentWriterContext) -> Result<usize> {
        if !self.is_dir() {
            return Err(FsError::NotDir);
        }
        let idx = ctx.pos();
        for entry in try_std!(self.path.read_dir()).skip(idx) {
            let entry = try_std!(entry);
            if let Err(e) = ctx.write_entry(
                &entry
                    .file_name()
                    .into_string()
                    .map_err(|_| FsError::InvalidParam)?,
                entry.ino(),
                entry
                    .file_type()
                    .map_err(|_| FsError::InvalidParam)?
                    .into_fs_filetype(),
            ) {
                if ctx.written_len() == 0 {
                    return Err(e);
                } else {
                    break;
                }
            };
        }
        Ok(ctx.written_len())
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
    fn open_file(&self) -> Result<MutexGuard<Option<fs::File>>> {
        let mut maybe_file = self.file.lock().unwrap();
        if maybe_file.is_none() {
            let file = try_std!(fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(&self.path));
            *maybe_file = Some(file);
        }
        Ok(maybe_file)
    }

    /// Returns `true` if this HNode is for a regular file.
    fn is_file(&self) -> bool {
        self.type_ == FileType::File
    }

    /// Returns `true` if this HNode is for a directory.
    fn is_dir(&self) -> bool {
        self.type_ == FileType::Dir
    }
}

pub trait IntoFsError {
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

trait IntoFsFileType {
    fn into_fs_filetype(self) -> FileType;
}

impl IntoFsFileType for fs::FileType {
    fn into_fs_filetype(self) -> FileType {
        if self.is_dir() {
            FileType::Dir
        } else if self.is_file() {
            FileType::File
        } else if self.is_symlink() {
            FileType::SymLink
        } else if self.is_block_device() {
            FileType::BlockDevice
        } else if self.is_char_device() {
            FileType::CharDevice
        } else if self.is_fifo() {
            FileType::NamedPipe
        } else if self.is_socket() {
            FileType::Socket
        } else {
            unimplemented!("unknown file type")
        }
    }
}

trait IntoFsMetadata {
    fn into_fs_metadata(self) -> Metadata;
}

impl IntoFsMetadata for fs::Metadata {
    fn into_fs_metadata(self) -> Metadata {
        use sgx_trts::libc;
        use std::os::linux::fs::MetadataExt;
        Metadata {
            dev: self.st_dev() as usize,
            inode: self.st_ino() as usize,
            size: self.st_size() as usize,
            blk_size: self.st_blksize() as usize,
            blocks: self.st_blocks() as usize,
            atime: Timespec {
                sec: self.st_atime(),
                nsec: self.st_atime_nsec(),
            },
            mtime: Timespec {
                sec: self.st_mtime(),
                nsec: self.st_mtime_nsec(),
            },
            ctime: Timespec {
                sec: self.st_ctime(),
                nsec: self.st_ctime_nsec(),
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
