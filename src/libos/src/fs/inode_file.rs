use super::file_ops::{ioctl::TcGets, ioctl::TcSets, NonBuiltinIoctlCmd};
use super::*;
use rcore_fs_sefs::dev::SefsMac;

// TODO: rename all INodeFile to InodeFile
pub use self::INodeFile as InodeFile;

pub struct INodeFile {
    inode: Arc<dyn INode>,
    open_path: String,
    offset: SgxMutex<usize>,
    access_mode: AccessMode,
    status_flags: RwLock<StatusFlags>,
}

impl INodeFile {
    pub fn open(inode: Arc<dyn INode>, flags: u32, open_path: String) -> Result<Self> {
        let access_mode = AccessMode::from_u32(flags)?;
        if (access_mode.readable() && !inode.allow_read()?) {
            return_errno!(EACCES, "File not readable");
        }
        if (access_mode.writable() && !inode.allow_write()?) {
            return_errno!(EACCES, "File not writable");
        }
        if access_mode.writable() && inode.metadata()?.type_ == FileType::Dir {
            return_errno!(EISDIR, "Directory cannot be open to write");
        }
        let creation_flags = CreationFlags::from_bits_truncate(flags);
        if creation_flags.should_truncate()
            && inode.metadata()?.type_ == FileType::File
            && access_mode.writable()
        {
            // truncate the length to 0
            inode.resize(0)?;
        }
        let status_flags = StatusFlags::from_bits_truncate(flags);
        Ok(INodeFile {
            inode,
            open_path,
            offset: SgxMutex::new(0),
            access_mode,
            status_flags: RwLock::new(status_flags),
        })
    }

    pub fn read(&self, buf: &mut [u8]) -> Result<usize> {
        if !self.access_mode.readable() {
            return_errno!(EBADF, "File not readable");
        }
        let mut offset = self.offset.lock().unwrap();
        let len = self.inode.read_at(*offset, buf).map_err(|e| errno!(e))?;
        *offset += len;
        Ok(len)
    }

    pub fn write(&self, buf: &[u8]) -> Result<usize> {
        if !self.access_mode.writable() {
            return_errno!(EBADF, "File not writable");
        }
        let mut offset = self.offset.lock().unwrap();
        if self.status_flags.read().unwrap().always_append() {
            let info = self.inode.metadata()?;
            *offset = info.size;
        }
        let len = self.inode.write_at(*offset, buf)?;
        *offset += len;
        Ok(len)
    }

    pub fn read_at(&self, offset: usize, buf: &mut [u8]) -> Result<usize> {
        if !self.access_mode.readable() {
            return_errno!(EBADF, "File not readable");
        }
        let len = self.inode.read_at(offset, buf)?;
        Ok(len)
    }

    pub fn write_at(&self, offset: usize, buf: &[u8]) -> Result<usize> {
        if !self.access_mode.writable() {
            return_errno!(EBADF, "File not writable");
        }
        let len = self.inode.write_at(offset, buf)?;
        Ok(len)
    }

    pub fn readv(&self, bufs: &mut [&mut [u8]]) -> Result<usize> {
        if !self.access_mode.readable() {
            return_errno!(EBADF, "File not readable");
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

    pub fn writev(&self, bufs: &[&[u8]]) -> Result<usize> {
        if !self.access_mode.writable() {
            return_errno!(EBADF, "File not writable");
        }
        let mut offset = self.offset.lock().unwrap();
        if self.status_flags.read().unwrap().always_append() {
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

    pub fn seek(&self, pos: SeekFrom) -> Result<usize> {
        let mut offset = self.offset.lock().unwrap();
        let new_offset: i64 = match pos {
            SeekFrom::Start(off /* as u64 */) => {
                if off > i64::max_value() as u64 {
                    return_errno!(EINVAL, "file offset is too large");
                }
                off as i64
            }
            SeekFrom::End(off /* as i64 */) => {
                let file_size = self.inode.metadata()?.size as i64;
                assert!(file_size >= 0);
                file_size
                    .checked_add(off)
                    .ok_or_else(|| errno!(EOVERFLOW, "file offset overflow"))?
            }
            SeekFrom::Current(off /* as i64 */) => (*offset as i64)
                .checked_add(off)
                .ok_or_else(|| errno!(EOVERFLOW, "file offset overflow"))?,
        };
        if new_offset < 0 {
            return_errno!(EINVAL, "file offset must not be negative");
        }
        // Invariant: 0 <= new_offset <= i64::max_value()
        let new_offset = new_offset as usize;
        *offset = new_offset;
        Ok(new_offset)
    }

    pub fn position(&self) -> usize {
        let offset = self.offset.lock().unwrap();
        *offset
    }

    pub fn flush(&self) -> Result<()> {
        self.inode.sync_data()?;
        Ok(())
    }

    pub fn access_mode(&self) -> AccessMode {
        self.access_mode
    }

    pub fn status_flags(&self) -> StatusFlags {
        let status_flags = self.status_flags.read().unwrap();
        *status_flags
    }

    /// Get the full path of the file when opened.
    ///
    /// Limitation: If file is renamed, the path will be invalid.
    pub fn open_path(&self) -> &str {
        &self.open_path
    }

    pub fn poll(&self, mask: Events, _poller: Option<&mut Poller>) -> Events {
        let events = match self.access_mode {
            AccessMode::O_RDONLY => Events::IN,
            AccessMode::O_WRONLY => Events::OUT,
            AccessMode::O_RDWR => Events::IN | Events::OUT,
        };
        events | mask
    }

    pub fn set_status_flags(&self, new_status_flags: StatusFlags) -> Result<()> {
        let mut status_flags = self.status_flags.write().unwrap();
        status_flags.remove(STATUS_FLAGS_MASK);
        status_flags.insert(new_status_flags & STATUS_FLAGS_MASK);
        Ok(())
    }

    pub fn ioctl(&self, cmd: &mut dyn IoctlCmd) -> Result<()> {
        async_io::match_ioctl_cmd_auto_error!(cmd, {
            cmd : NonBuiltinIoctlCmd => {
                self.inode.io_control(cmd.cmd_num().as_u32(), cmd.arg_ptr() as usize)?;
            },
        });
        Ok(())
    }

    pub fn test_range_lock(&self, lock: &mut RangeLock) -> Result<()> {
        let ext = match self.inode.ext() {
            Some(ext) => ext,
            None => {
                warn!("Inode extension is not supportted, the lock could be placed");
                lock.set_type(RangeLockType::F_UNLCK);
                return Ok(());
            }
        };

        match ext.get::<RangeLockList>() {
            None => {
                // The advisory lock could be placed if there is no lock list
                lock.set_type(RangeLockType::F_UNLCK);
            }
            Some(range_lock_list) => {
                range_lock_list.test_lock(lock);
            }
        }
        Ok(())
    }

    pub async fn set_range_lock(&self, lock: &RangeLock, is_nonblocking: bool) -> Result<()> {
        if RangeLockType::F_UNLCK == lock.type_() {
            return Ok(self.unlock_range_lock(lock));
        }

        self.check_range_lock_with_access_mode(lock)?;
        let ext = match self.inode.ext() {
            Some(ext) => ext,
            None => {
                warn!(
                    "Inode extension is not supported, let the lock could be acquired or released"
                );
                // TODO: Implement inode extension for FS
                return Ok(());
            }
        };
        let range_lock_list = match ext.get::<RangeLockList>() {
            Some(list) => list,
            None => ext.get_or_put_default::<RangeLockList>(),
        };

        range_lock_list.set_lock(lock, is_nonblocking).await?;
        Ok(())
    }

    pub fn release_range_locks(&self) {
        let range_lock = RangeLockBuilder::new()
            .owner(current!().process().pid() as _)
            .type_(RangeLockType::F_UNLCK)
            .range(FileRange::new(0, OFFSET_MAX).unwrap())
            .build()
            .unwrap();

        self.unlock_range_lock(&range_lock)
    }

    fn check_range_lock_with_access_mode(&self, lock: &RangeLock) -> Result<()> {
        match lock.type_() {
            RangeLockType::F_RDLCK => {
                if !self.access_mode.readable() {
                    return_errno!(EBADF, "File not readable");
                }
            }
            RangeLockType::F_WRLCK => {
                if !self.access_mode.writable() {
                    return_errno!(EBADF, "File not writable");
                }
            }
            _ => (),
        }
        Ok(())
    }

    fn unlock_range_lock(&self, lock: &RangeLock) {
        let ext = match self.inode.ext() {
            Some(ext) => ext,
            None => {
                return;
            }
        };
        let range_lock_list = match ext.get::<RangeLockList>() {
            Some(list) => list,
            None => {
                return;
            }
        };

        range_lock_list.unlock(lock)
    }

    pub async fn set_flock(&self, lock: Flock, is_nonblocking: bool) -> Result<()> {
        let ext = match self.inode.ext() {
            Some(ext) => ext,
            None => {
                warn!("Inode extension is not supported, let the lock could be acquired");
                // TODO: Implement inode extension for FS
                return Ok(());
            }
        };
        let flock_list = match ext.get::<FlockList>() {
            Some(list) => list,
            None => ext.get_or_put_default::<FlockList>(),
        };

        flock_list.set_lock(lock, is_nonblocking).await?;
        Ok(())
    }

    pub fn unlock_flock(&self) {
        let ext = match self.inode.ext() {
            Some(ext) => ext,
            None => {
                return;
            }
        };
        let flock_list = match ext.get::<FlockList>() {
            Some(list) => list,
            None => {
                return;
            }
        };

        flock_list.unlock(self);
    }

    pub fn iterate_entries(&self, writer: &mut dyn DirentWriter) -> Result<usize> {
        if !self.access_mode.readable() {
            return_errno!(EBADF, "File not readable. Can't read entry.");
        }
        let mut offset = self.offset.lock().unwrap();
        let mut dir_ctx = DirentWriterContext::new(*offset, writer);
        let written_size = self.inode.iterate_entries(&mut dir_ctx)?;
        *offset = dir_ctx.pos();
        Ok(written_size)
    }

    pub fn inode(&self) -> &Arc<dyn INode> {
        &self.inode
    }
}

impl Debug for INodeFile {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "INodeFile {{ inode: ???, open_path: {}, pos: {}, access_mode: {:?}, status_flags: {:#o} }}",
            self.open_path,
            *self.offset.lock().unwrap(),
            self.access_mode,
            *self.status_flags.read().unwrap()
        )
    }
}

impl Drop for INodeFile {
    fn drop(&mut self) {
        self.unlock_flock()
    }
}

pub trait INodeExt {
    fn read_as_vec(&self) -> Result<Vec<u8>>;
    fn allow_write(&self) -> Result<bool>;
    fn allow_read(&self) -> Result<bool>;
}

impl INodeExt for dyn INode {
    fn read_as_vec(&self) -> Result<Vec<u8>> {
        let size = self.metadata()?.size;
        let mut buf = Vec::with_capacity(size);
        unsafe {
            buf.set_len(size);
        }
        self.read_at(0, buf.as_mut_slice())?;
        Ok(buf)
    }

    fn allow_write(&self) -> Result<bool> {
        let info = self.metadata()?;
        let file_mode = FileMode::from_bits_truncate(info.mode);
        Ok(file_mode.is_writable())
    }

    fn allow_read(&self) -> Result<bool> {
        let info = self.metadata()?;
        let file_mode = FileMode::from_bits_truncate(info.mode);
        Ok(file_mode.is_readable())
    }
}
