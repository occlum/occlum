use super::sync_fs_wrapper::SyncInode;
use super::*;
use crate::process::do_getuid::do_getuid;

use async_mountfs::AsyncMInode;
use async_rt::sync::Mutex as AsyncMutex;
use async_trait::async_trait;

/// The opened async inode through sys_open()
pub struct AsyncFileHandle {
    dentry: Dentry,
    offset: AsyncMutex<usize>,
    access_mode: AccessMode,
    status_flags: RwLock<StatusFlags>,
}

impl AsyncFileHandle {
    pub async fn open(
        dentry: Dentry,
        access_mode: AccessMode,
        creation_flags: CreationFlags,
        status_flags: StatusFlags,
    ) -> Result<Self> {
        if access_mode.writable() && dentry.inode().metadata().await?.type_ == FileType::Dir {
            return_errno!(EISDIR, "Directory cannot be open to write");
        }
        if creation_flags.should_truncate()
            && dentry.inode().metadata().await?.type_ == FileType::File
            && access_mode.writable()
        {
            // truncate the length to 0
            dentry.inode().resize(0).await?;
        }
        Ok(Self {
            dentry,
            offset: AsyncMutex::new(0),
            access_mode,
            status_flags: RwLock::new(status_flags),
        })
    }

    pub async fn read(&self, buf: &mut [u8]) -> Result<usize> {
        if !self.access_mode.readable() {
            return_errno!(EBADF, "File not readable");
        }
        let mut offset = self.offset.lock().await;
        let len = self.dentry.inode().read_at(*offset, buf).await?;
        *offset += len;
        Ok(len)
    }

    pub async fn readv(&self, bufs: &mut [&mut [u8]]) -> Result<usize> {
        if !self.access_mode.readable() {
            return_errno!(EBADF, "File not readable");
        }
        let mut offset = self.offset.lock().await;
        let mut total_len = 0;
        for buf in bufs {
            match self.dentry.inode().read_at(*offset, buf).await {
                Ok(len) => {
                    total_len += len;
                    *offset += len;
                }
                Err(_) if total_len != 0 => break,
                Err(e) => return Err(e),
            }
        }
        Ok(total_len)
    }

    pub async fn preadv(&self, bufs: &mut [&mut [u8]], offset: usize) -> Result<usize> {
        if !self.access_mode.readable() {
            return_errno!(EBADF, "File not readable");
        }
        let mut offset = offset;
        let mut total_len = 0;
        for buf in bufs {
            match self.dentry.inode().read_at(offset, buf).await {
                Ok(len) => {
                    total_len += len;
                    offset += len;
                }
                Err(_) if total_len != 0 => break,
                Err(e) => return Err(e),
            }
        }
        Ok(total_len)
    }

    pub async fn pread(&self, buf: &mut [u8], offset: usize) -> Result<usize> {
        if !self.access_mode().readable() {
            return_errno!(EBADF, "file is not readable");
        }
        self.dentry.inode().read_at(offset, buf).await
    }

    pub async fn write(&self, buf: &[u8]) -> Result<usize> {
        if !self.access_mode.writable() {
            return_errno!(EBADF, "File not writable");
        }
        let mut offset = self.offset.lock().await;
        if self.status_flags.read().unwrap().always_append() {
            let info = self.dentry.inode().metadata().await?;
            *offset = info.size;
        }
        let len = self.dentry.inode().write_at(*offset, buf).await?;
        *offset += len;
        Ok(len)
    }

    pub async fn writev(&self, bufs: &[&[u8]]) -> Result<usize> {
        if !self.access_mode.writable() {
            return_errno!(EBADF, "File not writable");
        }
        let mut offset = self.offset.lock().await;
        if self.status_flags.read().unwrap().always_append() {
            let info = self.dentry.inode().metadata().await?;
            *offset = info.size;
        }
        let mut total_len = 0;
        for buf in bufs {
            match self.dentry.inode().write_at(*offset, buf).await {
                Ok(len) => {
                    total_len += len;
                    *offset += len;
                }
                Err(_) if total_len != 0 => break,
                Err(e) => return Err(e),
            }
        }
        Ok(total_len)
    }

    pub async fn pwritev(&self, bufs: &[&[u8]], offset: usize) -> Result<usize> {
        if !self.access_mode.writable() {
            return_errno!(EBADF, "File not writable");
        }
        let mut offset = offset;
        let mut total_len = 0;
        for buf in bufs {
            match self.dentry.inode().write_at(offset, buf).await {
                Ok(len) => {
                    total_len += len;
                    offset += len;
                }
                Err(_) if total_len != 0 => break,
                Err(e) => return Err(e),
            }
        }
        Ok(total_len)
    }

    pub async fn pwrite(&self, buf: &[u8], offset: usize) -> Result<usize> {
        if !self.access_mode().writable() {
            return_errno!(EBADF, "file is not writable");
        }
        self.dentry.inode().write_at(offset, buf).await
    }

    pub async fn seek(&self, pos: SeekFrom) -> Result<usize> {
        let mut offset = self.offset.lock().await;
        let new_offset: i64 = match pos {
            SeekFrom::Start(off /* as u64 */) => {
                if off > i64::max_value() as u64 {
                    return_errno!(EINVAL, "file offset is too large");
                }
                off as i64
            }
            SeekFrom::End(off /* as i64 */) => {
                let file_size = self.dentry.inode().metadata().await?.size as i64;
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

    pub async fn offset(&self) -> usize {
        let offset = self.offset.lock().await;
        *offset
    }

    pub fn poll(&self, mask: Events, poller: Option<&Poller>) -> Events {
        self.dentry().inode().poll(mask, poller)
    }

    pub fn register_observer(&self, _observer: Arc<dyn Observer>, _mask: Events) -> Result<()> {
        return_errno!(EINVAL, "do not support observers");
    }

    pub fn unregister_observer(&self, _observer: &Arc<dyn Observer>) -> Result<Arc<dyn Observer>> {
        return_errno!(EINVAL, "do not support observers");
    }

    pub fn access_mode(&self) -> AccessMode {
        self.access_mode
    }

    pub fn status_flags(&self) -> StatusFlags {
        let status_flags = self.status_flags.read().unwrap();
        *status_flags
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

    pub async fn ioctl(&self, cmd: &mut dyn IoctlCmd) -> Result<()> {
        self.dentry().inode().ioctl(cmd).await
    }

    pub async fn iterate_entries(&self, writer: &mut dyn DirentWriter) -> Result<usize> {
        if !self.access_mode.readable() {
            return_errno!(EBADF, "File not readable. Can't read entry.");
        }
        let mut offset = self.offset.lock().await;
        let mut dir_ctx = DirentWriterContext::new(*offset, writer);
        let written_size = self.dentry.inode().iterate_entries(&mut dir_ctx).await?;
        *offset = dir_ctx.pos();
        Ok(written_size)
    }

    pub fn test_range_lock(&self, lock: &mut RangeLock) -> Result<()> {
        let ext = match self.dentry().inode().ext() {
            Some(ext) => ext,
            None => {
                warn!("Inode extension is not supported, the lock could be placed");
                lock.set_type(RangeLockType::F_UNLCK);
                return Ok(());
            }
        };
        match ext.get::<RangeLockList>() {
            None => {
                // The advisory lock could be placed if there is no list
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
        let ext = match self.dentry().inode().ext() {
            Some(ext) => ext,
            None => {
                warn!(
                    "Inode extension is not supported, let the lock could be acquired or released"
                );
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

    fn unlock_range_lock(&self, lock: &RangeLock) {
        let ext = match self.dentry().inode().ext() {
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

    pub async fn set_flock(&self, lock: Flock, is_nonblocking: bool) -> Result<()> {
        let ext = match self.dentry().inode().ext() {
            Some(ext) => ext,
            None => {
                warn!("Inode extension is not supported, let the lock could be acquired");
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
        let ext = match self.dentry().inode().ext() {
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

    pub fn dentry(&self) -> &Dentry {
        &self.dentry
    }
}

impl std::fmt::Debug for AsyncFileHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "AsyncFileHandle {{ dentry: ???, offset: ???, access_mode: {:?}, status_flags: {:#o} }}",
            self.access_mode,
            *self.status_flags.read().unwrap()
        )
    }
}

impl Drop for AsyncFileHandle {
    fn drop(&mut self) {
        self.unlock_flock()
    }
}

#[async_trait]
pub trait AsyncInodeExt {
    async fn allow_write(&self) -> bool;
    async fn allow_read(&self) -> bool;
    async fn read_elf64_lazy_as_vec(&self) -> Result<Vec<u8>>;
    // Workaround to convert AsyncInode to sync INode
    fn as_sync_inode(&self) -> Option<&Arc<dyn INode>>;
}

#[async_trait]
impl AsyncInodeExt for dyn AsyncInode {
    async fn allow_write(&self) -> bool {
        // TODO: Since Occlum does not support the capability,
        //       just skip the permission check if uid is root.
        if do_getuid() == 0 {
            return true;
        }

        let info = self.metadata().await.unwrap();
        let file_mode = FileMode::from_bits_truncate(info.mode);
        file_mode.is_writable()
    }

    async fn allow_read(&self) -> bool {
        // TODO: See the comments in allow_write
        if do_getuid() == 0 {
            return true;
        }

        let info = self.metadata().await.unwrap();
        let file_mode = FileMode::from_bits_truncate(info.mode);
        file_mode.is_readable()
    }

    async fn read_elf64_lazy_as_vec(&self) -> Result<Vec<u8>> {
        let size = self.metadata().await.unwrap().size;
        let mut buf = vec![0; size];
        let elf64_hdr_size = 64;
        self.read_at(
            0,
            &mut buf.as_mut_slice()[..core::cmp::min(elf64_hdr_size, size)],
        )
        .await?;
        Ok(buf)
    }

    fn as_sync_inode(&self) -> Option<&Arc<dyn INode>> {
        let mnt_inode = self.downcast_ref::<AsyncMInode>();
        if mnt_inode.is_none() {
            return None;
        }
        let mut mnt_inode = mnt_inode.unwrap();
        loop {
            if let Some(inode) = mnt_inode.inner().downcast_ref::<SyncInode>() {
                return Some(inode.inner());
            } else if let Some(inode) = mnt_inode.inner().downcast_ref::<AsyncMInode>() {
                mnt_inode = inode;
            } else {
                break;
            }
        }
        None
    }
}
