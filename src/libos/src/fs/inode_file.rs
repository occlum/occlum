use super::file_ops::NonBuiltinIoctlCmd;
use super::*;
use rcore_fs_sefs::dev::SefsMac;

// TODO: rename all INodeFile to InodeFile
pub use self::INodeFile as InodeFile;

pub struct INodeFile {
    inode: Arc<dyn INode>,
    abs_path: String,
    offset: SgxMutex<usize>,
    access_mode: AccessMode,
    status_flags: RwLock<StatusFlags>,
}

impl INodeFile {
    pub fn open(inode: Arc<dyn INode>, abs_path: &str, flags: u32) -> Result<Self> {
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
        let status_flags = StatusFlags::from_bits_truncate(flags);
        Ok(INodeFile {
            inode,
            abs_path: abs_path.to_owned(),
            offset: SgxMutex::new(0),
            access_mode,
            status_flags: RwLock::new(status_flags),
        })
    }

    pub fn abs_path(&self) -> &str {
        &self.abs_path
    }

    pub fn read(&self, buf: &mut [u8]) -> Result<usize> {
        if !self.access_mode.readable() {
            return_errno!(EACCES, "File not readable");
        }
        let mut offset = self.offset.lock().unwrap();
        let len = self.inode.read_at(*offset, buf).map_err(|e| errno!(e))?;
        *offset += len;
        Ok(len)
    }

    pub fn write(&self, buf: &[u8]) -> Result<usize> {
        if !self.access_mode.writable() {
            return_errno!(EACCES, "File not writable");
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
            return_errno!(EACCES, "File not readable");
        }
        let len = self.inode.read_at(offset, buf)?;
        Ok(len)
    }

    pub fn write_at(&self, offset: usize, buf: &[u8]) -> Result<usize> {
        if !self.access_mode.writable() {
            return_errno!(EACCES, "File not writable");
        }
        let len = self.inode.write_at(offset, buf)?;
        Ok(len)
    }

    pub fn readv(&self, bufs: &mut [&mut [u8]]) -> Result<usize> {
        if !self.access_mode.readable() {
            return_errno!(EACCES, "File not readable");
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
            return_errno!(EACCES, "File not writable");
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
        // Currently, F_SETFL can change only the O_APPEND,
        // O_ASYNC, O_NOATIME, and O_NONBLOCK flags
        let valid_flags_mask = StatusFlags::O_APPEND
            | StatusFlags::O_ASYNC
            | StatusFlags::O_NOATIME
            | StatusFlags::O_NONBLOCK;
        status_flags.remove(valid_flags_mask);
        status_flags.insert(new_status_flags & valid_flags_mask);
        Ok(())
    }

    pub fn ioctl(&self, cmd: &mut dyn IoctlCmd) -> Result<()> {
        async_io::match_ioctl_cmd_mut!(cmd, {
            cmd : NonBuiltinIoctlCmd => {
                self.inode.io_control(cmd.cmd_num().as_u32(), cmd.arg_ptr() as usize)?;
            },
            _ => {
                return_errno!(EINVAL, "unsupported ioctl cmd");
            }
        });
        Ok(())
    }

    pub fn test_advisory_lock(&self, lock: &mut Flock) -> Result<()> {
        // Let the advisory lock could be placed
        // TODO: Implement the real advisory lock
        lock.l_type = FlockType::F_UNLCK;
        Ok(())
    }

    pub fn set_advisory_lock(&self, lock: &Flock) -> Result<()> {
        match lock.l_type {
            FlockType::F_RDLCK => {
                if !self.access_mode.readable() {
                    return_errno!(EACCES, "File not readable");
                }
            }
            FlockType::F_WRLCK => {
                if !self.access_mode.writable() {
                    return_errno!(EACCES, "File not writable");
                }
            }
            _ => (),
        }
        // Let the advisory lock could be acquired or released
        // TODO: Implement the real advisory lock
        Ok(())
    }

    pub fn inode(&self) -> &dyn INode {
        &*self.inode as _
    }
}

impl Debug for INodeFile {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "INodeFile {{ inode: ???, abs_path: {}, pos: {}, access_mode: {:?}, status_flags: {:#o} }}",
            self.abs_path,
            *self.offset.lock().unwrap(),
            self.access_mode,
            *self.status_flags.read().unwrap()
        )
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
