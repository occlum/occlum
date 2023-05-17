use super::*;

use crate::fs::FileMode;
use crate::process::do_getuid::{do_getegid, do_geteuid};
use crate::process::{gid_t, uid_t, ThreadRef};
use crate::time::{do_gettimeofday, time_t};
use crate::vm::{
    ChunkRef, VMInitializer, VMMapOptionsBuilder, VMPerms, VMRange, USER_SPACE_VM_MANAGER,
};
use std::collections::{HashMap, HashSet};

#[allow(non_camel_case_types)]
pub type key_t = u32;
pub type ShmId = u32;
pub type CmdId = u32;

// min shared seg size (bytes)
const SHMMIN: usize = 1;
// max shared seg size (bytes)
const SHMMAX: usize = (usize::MAX - (1_usize << 24));
// max num of segs system wide,
// also indicates the max shmid - 1 in Occlum
const SHMMNI: ShmId = 4096;

const IPC_PRIVATE: key_t = 0;

// For cmd in shmctl()
const IPC_RMID: CmdId = 0;
const IPC_SET: CmdId = 1;
const IPC_STAT: CmdId = 2;
const IPC_INFO: CmdId = 3;
const SHM_LOCK: CmdId = 11;
const SHM_UNLOCK: CmdId = 12;
const SHM_STAT: CmdId = 13;
const SHM_INFO: CmdId = 14;
const SHM_STAT_ANY: CmdId = 15;

#[allow(non_camel_case_types)]
#[derive(Debug)]
#[repr(C)]
struct ipc_perm_t {
    key: key_t,
    uid: uid_t,
    gid: gid_t,
    cuid: uid_t,
    cgid: gid_t,
    mode: u16,
    pad1: u16,
    seq: u16,
    pad2: u16,
    unused1: u64,
    unused2: u64,
}

#[allow(non_camel_case_types)]
#[derive(Debug)]
#[repr(C)]
pub struct shmids_t {
    shm_perm: ipc_perm_t,
    shm_segsz: size_t,
    shm_atime: time_t,
    shm_dtime: time_t,
    shm_ctime: time_t,
    shm_cpid: pid_t,
    shm_lpid: pid_t,
    shm_nattach: u64,
    unused1: u64,
    unused2: u64,
}

// shared memory segment status
bitflags! {
    struct ShmStatus : u16 {
        // segment will be destroyed on last detach
        const SHM_DEST = 0o1000;
        // segment will not be swapped, unused now
        const SHM_LOCKED = 0o2000;
    }
}

bitflags! {
    pub struct ShmFlags: u32 {
        const IPC_CREAT = 0o1000;
        const IPC_EXCL = 0o2000;

        /// read by owner
        const S_IRUSR = FileMode::S_IRUSR.bits() as u32;
        /// write by owner
        const S_IWUSR = FileMode::S_IWUSR.bits() as u32;
        /// execute/search by owner
        const S_IXUSR = FileMode::S_IXUSR.bits() as u32;
        /// read by group
        const S_IRGRP = FileMode::S_IRGRP.bits() as u32;
        /// write by group
        const S_IWGRP = FileMode::S_IWGRP.bits() as u32;
        /// execute/search by group
        const S_IXGRP = FileMode::S_IXGRP.bits() as u32;
        /// read by others
        const S_IROTH = FileMode::S_IROTH.bits() as u32;
        /// write by others
        const S_IWOTH = FileMode::S_IWOTH.bits() as u32;
        /// execute/search by others
        const S_IXOTH = FileMode::S_IXOTH.bits() as u32;
    }
}

impl ShmFlags {
    fn to_file_mode(&self) -> FileMode {
        let mut shmflgs = *self;
        shmflgs.remove(ShmFlags::IPC_CREAT);
        shmflgs.remove(ShmFlags::IPC_EXCL);
        let file_mode = FileMode::from_bits(shmflgs.bits as u16 & FileMode::all().bits()).unwrap();
        file_mode
    }
}

#[derive(Debug)]
struct ShmSegment {
    shmid: ShmId,
    key: key_t,

    uid: uid_t,
    gid: gid_t,
    cuid: uid_t,
    cgid: gid_t,
    mode: FileMode,
    status: ShmStatus,

    shm_atime: time_t,
    shm_dtime: time_t,
    shm_ctime: time_t,

    shm_cpid: pid_t,
    shm_lpid: pid_t,

    shm_nattach: u64,
    chunk: ChunkRef,
    process_set: HashSet<pid_t>,
}

impl ShmSegment {
    async fn new(shmid: ShmId, key: key_t, size: size_t, mode: FileMode) -> Result<Self> {
        let vm_option = VMMapOptionsBuilder::default()
            .size(size)
            // Currently, Occlum only support shared memory segment with rw permission
            .perms(VMPerms::READ | VMPerms::WRITE)
            .initializer(VMInitializer::FillZeros())
            .build()?;
        let chunk = USER_SPACE_VM_MANAGER
            .internal()
            .await
            .mmap_chunk(&vm_option)
            .await?;

        Ok(ShmSegment {
            shmid: shmid,
            key: key,
            uid: do_geteuid() as u32,
            cuid: do_geteuid() as u32,
            gid: do_getegid() as u32,
            cgid: do_getegid() as u32,
            mode: mode,
            status: ShmStatus::empty(),
            shm_atime: 0,
            shm_dtime: 0,
            shm_ctime: ShmManager::current_time(),
            shm_cpid: current!().process().pid(),
            shm_lpid: 0,
            shm_nattach: 0,
            chunk: chunk,
            process_set: HashSet::new(),
        })
    }

    fn check_perm(&self) -> Result<()> {
        // TODO: Add permission control
        Ok(())
    }

    fn set_destruction(&mut self) {
        self.status.insert(ShmStatus::SHM_DEST)
    }

    fn is_destruction(&self) -> bool {
        self.status.contains(ShmStatus::SHM_DEST)
    }

    fn shm_start(&self) -> usize {
        self.chunk.range().start()
    }

    fn shm_size(&self) -> usize {
        self.chunk.range().size()
    }

    fn shm_add_pid(&mut self, pid: &pid_t) -> Result<()> {
        if self.process_set.contains(pid) {
            return_errno!(EINVAL, "this pid has been attached to the shm");
        }
        self.process_set.insert(*pid);
        Ok(())
    }

    fn shm_remove_pid(&mut self, pid: &pid_t) -> Result<()> {
        if !self.process_set.contains(pid) {
            return_errno!(EINVAL, " this pid has not been attached to the shm");
        }
        self.process_set.remove(&pid);
        Ok(())
    }
}

impl Drop for ShmSegment {
    fn drop(&mut self) {
        debug!("drop shm: {:?}", self);
        assert!(self.shm_nattach == 0);
        assert!(self.process_set.is_empty());
    }
}

#[derive(Debug)]
struct ShmIdManager {
    used_id: HashSet<ShmId>,
    free_num: u32,
    last_alloc_id: ShmId,
}

impl ShmIdManager {
    fn new() -> Self {
        let used_id = HashSet::new();
        let free_num = SHMMNI as u32;
        let last_alloc_id = SHMMNI - 1;
        ShmIdManager {
            used_id,
            free_num,
            last_alloc_id,
        }
    }

    // Always return next free id for shmid
    fn get_new_shmid(&mut self) -> Result<ShmId> {
        if self.free_num == 0 {
            return_errno!(ENOSPC, "all possible shared memory IDs have been taken");
        } else {
            self.free_num -= 1;
        }
        let mut id = self.last_alloc_id + 1;
        loop {
            if id == SHMMNI {
                id = 0;
            }
            if !self.used_id.contains(&id) {
                break;
            }
            id += 1;
        }
        self.last_alloc_id = id;
        Ok(id)
    }

    fn free_shmid(&mut self, shmid: &ShmId) -> Result<()> {
        self.free_num += 1;
        self.used_id.remove(shmid);
        Ok(())
    }
}

lazy_static! {
    pub static ref SHM_MANAGER: ShmManager = ShmManager::new();
}

#[derive(Debug)]
pub struct ShmManager {
    shm_segments: RwLock<HashMap<ShmId, ShmSegment>>,
    shmid_manager: RwLock<ShmIdManager>,
}

impl ShmManager {
    fn new() -> Self {
        ShmManager {
            shm_segments: RwLock::new(HashMap::new()),
            shmid_manager: RwLock::new(ShmIdManager::new()),
        }
    }

    fn current_time() -> time_t {
        do_gettimeofday().sec()
    }

    fn get_new_shmid(&self) -> Result<ShmId> {
        let mut shmid_manager = self.shmid_manager.write().unwrap();
        shmid_manager.get_new_shmid()
    }

    fn free_shmid(&self, shmid: &ShmId) -> Result<()> {
        let mut shmid_manager = self.shmid_manager.write().unwrap();
        shmid_manager.free_shmid(&shmid)
    }

    async fn shmctl_rmshm(&self, shmid: ShmId) -> Result<()> {
        let mut shm_segments = self.shm_segments.write().unwrap();
        let shm = shm_segments.get_mut(&shmid);

        if let Some(shm) = shm {
            shm.shm_ctime = ShmManager::current_time();
            shm.set_destruction();
            if shm.shm_nattach == 0 {
                let shmid = shm.shmid;
                self.free_shmid(&shmid)?;
                if let Some(shm_segment) = shm_segments.remove(&shmid) {
                    USER_SPACE_VM_MANAGER
                        .internal()
                        .await
                        .munmap_chunk(&shm_segment.chunk, None)
                        .await;
                }
            }
        } else {
            return_errno!(EINVAL, "cannot find shm by shmid");
        }
        Ok(())
    }

    fn shmctl_ipcstat(&self, shmid: ShmId, buf: Option<&mut shmids_t>) -> Result<()> {
        let shm_segments = self.shm_segments.read().unwrap();
        let shm = shm_segments.get(&shmid);
        if let Some(shm) = shm {
            shm.check_perm()?;
            let mut buf = match buf {
                Some(buf) => buf,
                None => return_errno!(EFAULT, "buf is empty"),
            };
            let shm_perm = ipc_perm_t {
                key: shm.key,
                uid: shm.uid,
                gid: shm.uid,
                cuid: shm.cuid,
                cgid: shm.cgid,
                mode: shm.mode.bits() | shm.status.bits(),
                pad1: 0,
                seq: 0,
                pad2: 0,
                unused1: 0,
                unused2: 0,
            };
            let shmids = shmids_t {
                shm_perm: shm_perm,
                shm_segsz: shm.shm_size(),
                shm_atime: shm.shm_atime,
                shm_dtime: shm.shm_dtime,
                shm_ctime: shm.shm_ctime,
                shm_cpid: shm.shm_cpid,
                shm_lpid: shm.shm_lpid,
                shm_nattach: shm.shm_nattach,
                unused1: 0,
                unused2: 0,
            };
            *buf = shmids;
        } else {
            return_errno!(EINVAL, "cannot find shm by shmid");
        }
        Ok(())
    }

    pub async fn do_shmget(&self, key: key_t, size: usize, shmflg: ShmFlags) -> Result<ShmId> {
        debug!(
            "do_shmget: key: {:?}, size: {:?}, shmflg: {:?}",
            key, size, shmflg
        );

        // Check the size from user for shm creation
        if shmflg.contains(ShmFlags::IPC_CREAT) && (size < SHMMIN || size > SHMMAX) {
            return_errno!(EINVAL, "invalid size");
        }

        let mut mode = shmflg.to_file_mode();
        // The creator and user must have the read and write permission to the segment
        let read_per = mode.contains(FileMode::S_IRUSR)
            || mode.contains(FileMode::S_IRGRP)
            || mode.contains(FileMode::S_IROTH);
        let write_per = mode.contains(FileMode::S_IWUSR)
            || mode.contains(FileMode::S_IWGRP)
            || mode.contains(FileMode::S_IWOTH);
        if !(read_per && write_per) {
            warn!("shared memory segement in occlum should have rw permission");
        }

        let mut shm_segments = self.shm_segments.write().unwrap();
        let shmid = if key == IPC_PRIVATE {
            let shmid = self.get_new_shmid()?;
            let shm = ShmSegment::new(shmid, key, size, mode).await?;
            shm_segments.insert(shm.shmid, shm);
            shmid
        } else {
            // Get the shm from key if the segment is not marked to be destroyed
            let shm = shm_segments
                .values()
                .find(|&shm| !shm.is_destruction() && shm.key == key);
            let shmid = if let Some(shm) = shm {
                if shmflg.contains(ShmFlags::IPC_CREAT) && shmflg.contains(ShmFlags::IPC_EXCL) {
                    return_errno!(
                        EEXIST,
                        "the shared memory segment already exists for given key"
                    );
                }
                // The size from user must be less than the actual size
                if size > shm.shm_size() {
                    return_errno!(EINVAL, "the size from user is too large");
                }
                // Check the permission
                shm.check_perm()?;
                shm.shmid
            } else {
                if !shmflg.contains(ShmFlags::IPC_CREAT) {
                    return_errno!(ENOENT, "no segment exists for given key");
                }
                let shmid = self.get_new_shmid()?;
                let shm = ShmSegment::new(shmid, key, size, mode).await?;
                shm_segments.insert(shm.shmid, shm);
                shmid
            };
            shmid
        };
        Ok(shmid)
    }

    pub fn do_shmat(&self, shmid: ShmId, addr: usize, shmflg: ShmFlags) -> Result<usize> {
        debug!(
            "do_shmat: shmid: {:?}, addr: {:?}, shmflg: {:?}",
            shmid, addr, shmflg
        );
        let pid = current!().process().pid();
        let mut shm_segments = self.shm_segments.write().unwrap();
        let shm = shm_segments.get_mut(&shmid);
        let addr = if let Some(shm) = shm {
            if addr != 0 && addr != shm.shm_start() {
                return_errno!(EINVAL, "invalid addr");
            }
            // Check the permission
            shm.check_perm()?;

            shm.shm_nattach += 1;
            shm.shm_atime = ShmManager::current_time();
            shm.shm_add_pid(&pid)?;
            shm.shm_lpid = pid;
            shm.shm_start()
        } else {
            return_errno!(EINVAL, "cannot find shm by shmid");
        };
        Ok(addr)
    }

    pub async fn do_shmdt(&self, addr: usize) -> Result<()> {
        debug!("do_shmdt: addr: {:?}", addr);
        let pid = current!().process().pid();
        let mut shm_segments = self.shm_segments.write().unwrap();
        let shm = shm_segments
            .values_mut()
            .find(|shm| shm.shm_start() == addr);

        if let Some(shm) = shm {
            shm.shm_dtime = ShmManager::current_time();
            shm.shm_lpid = pid;
            shm.shm_remove_pid(&pid)?;
            shm.shm_nattach -= 1;
            if shm.is_destruction() && shm.shm_nattach == 0 {
                let shmid = shm.shmid;
                self.free_shmid(&shmid);
                if let Some(shm_segment) = shm_segments.remove(&shmid) {
                    USER_SPACE_VM_MANAGER
                        .internal()
                        .await
                        .munmap_chunk(&shm_segment.chunk, None)
                        .await;
                }
            }
        } else {
            return_errno!(EINVAL, "cannot find shm by addr");
        }
        Ok(())
    }

    pub async fn do_shmctl(
        &self,
        shmid: ShmId,
        cmd: CmdId,
        buf: Option<&mut shmids_t>,
    ) -> Result<()> {
        debug!(
            "do_shmctl: shmid: {:?}, cmd: {:?}, buf: {:?}",
            shmid, cmd, buf
        );
        match cmd {
            IPC_RMID => self.shmctl_rmshm(shmid).await,
            IPC_STAT => self.shmctl_ipcstat(shmid, buf),
            _ => return_errno!(EINVAL, "unimplemented cmd"),
        }
    }

    pub async fn detach_shm_when_process_exit(&self, thread: &ThreadRef) {
        let pid = &thread.process().pid();
        let mut shm_segments = self.shm_segments.write().unwrap();
        let freed_shm_segments = shm_segments
            .drain_filter(|shmid, shm| match shm.shm_remove_pid(&pid) {
                // There exists a shm that has been attached to the current process
                Ok(_) => {
                    shm.shm_nattach -= 1;
                    if shm.is_destruction() && shm.shm_nattach == 0 {
                        self.free_shmid(&shmid);
                        true
                    } else {
                        false
                    }
                }
                Err(_) => false,
            })
            .map(|(_, shm)| shm)
            .collect::<Vec<_>>();

        for shm in freed_shm_segments {
            USER_SPACE_VM_MANAGER
                .internal()
                .await
                .munmap_chunk(&shm.chunk, None)
                .await;
        }
    }

    pub async fn clean_when_libos_exit(&self) {
        let mut shm_segments = self.shm_segments.write().unwrap();
        for (_, shm) in shm_segments.drain() {
            USER_SPACE_VM_MANAGER
                .internal()
                .await
                .munmap_single_vma_chunk(&shm.chunk, None)
                .await;
            debug!("clean shm: {:?}", shm);
            self.free_shmid(&shm.shmid);
        }
    }
}
