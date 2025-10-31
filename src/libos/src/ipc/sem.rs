use super::*;

use crate::events::{Observer, Waiter, WaiterQueue};
use crate::process::{do_getegid, do_geteuid, gid_t, pid_t, uid_t, ThreadRef};
use crate::time::{do_gettimeofday, time_t};
use crate::util::mem_util::from_user;
use alloc::vec::Vec;
use bitflags::bitflags;
use core::sync::atomic::{AtomicBool, AtomicI32, AtomicI64, Ordering};
use intrusive_collections::LinkedList;
use std::cmp::Ordering as CmpOrdering;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::time::Duration;
use std::{cmp, time};

#[allow(non_camel_case_types)]
pub type key_t = u32;
pub type SemId = u32;
pub type CmdId = u32;

const IPC_PRIVATE: key_t = 0;

// Maximum number of semaphore sets
const SEMMNI: SemId = 128;
// Maximum semaphores per set
const SEMMSL: usize = 250;
// Maximum semaphores system-wide
const SEMMNS: usize = SEMMNI as usize * SEMMSL;
// Maximum operations per semop call
const SEMOPM: usize = 32;
// Maximum semaphore value
const SEMVMX: usize = 32767;

const IPC_RMID: CmdId = 0; // Remove semaphore set
const IPC_SET: CmdId = 1; // Set semaphore set parameters
const IPC_STAT: CmdId = 2; // Get semaphore set status
const IPC_INFO: CmdId = 3; // Get system-wide semaphore information

// Semaphore operation command constants (following System V specifications)
const SEM_GETPID: CmdId = 11; // Get PID of last operation
const SEM_GETVAL: CmdId = 12; // Get semaphore value
const SEM_GETALL: CmdId = 13; // Get all semaphore values in set
const SEM_GETNCNT: CmdId = 14; // Get count of processes waiting for value > current
const SEM_GETZCNT: CmdId = 15; // Get count of processes waiting for value = 0
const SEM_SETVAL: CmdId = 16; // Set semaphore value
const SEM_SETALL: CmdId = 17; // Set all semaphore values in set
const SEM_STAT: CmdId = 18; // Get status by semid
const SEM_INFO: CmdId = 19; // Get extended semaphore information
const SEM_UNDO: CmdId = 20; // Undo operations for current process

bitflags! {
    pub struct SemFlags: u32 {
        // IPC operation flags
        const IPC_CREAT = 0o1000;    // Create if not exists
        const IPC_EXCL = 0o2000;     // Fail if exists (with IPC_CREAT)
        const IPC_NOWAIT = 0o4000;   // Non-blocking mode

        // Semaphore-specific flags
        const SEM_UNDO = 0o10000;    // Auto-undo operations on process exit

        // Permission flags
        const S_IRUSR = 0o400;       // Owner read permission
        const S_IWUSR = 0o200;       // Owner write permission
        const S_IXUSR = 0o100;       // Owner execute permission

        const S_IRGRP = 0o040;       // Group read permission
        const S_IWGRP = 0o020;       // Group write permission
        const S_IXGRP = 0o010;       // Group execute permission

        const S_IROTH = 0o004;       // Others read permission
        const S_IWOTH = 0o002;       // Others write permission
        const S_IXOTH = 0o001;       // Others execute permission
    }
}

#[allow(non_camel_case_types)]
#[derive(Debug)]
#[repr(C)]
pub struct semids_t {
    sem_perm: ipc_perm_t,
    sem_otime: time_t,
    sem_otime_high: time_t,
    sem_ctime: time_t,
    sem_ctime_high: time_t,
    sem_nsems: u64,
    unused1: u64,
    unused2: u64,
}

/// Structure for storing semaphore system limits (used with IPC_INFO)
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy)]
#[repr(C)]
struct seminfo_t {
    semmap: u32, // Number of entries in semaphore map
    semmni: u32, // Maximum number of semaphore sets
    semmns: u32, // Maximum number of semaphores in system
    semmnu: u32, // System-wide maximum number of undo structures
    semmsl: u32, // Maximum number of semaphores per set
    semopm: u32, // Maximum number of operations per semop call
    semume: u32, // Maximum number of undo entries per process
    semusz: u32, // Size in bytes of undo structure
    semvmx: u32, // Maximum semaphore value
    semaem: u32, // Adjust on exit max value
}

/// Structure for extended semaphore system information (used with SEM_INFO)
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy)]
#[repr(C)]
struct seminfo_ext_t {
    sem_info: seminfo_t, // Basic limit information
    semusz: u32,         // Size of sem_undo structure
    semaem: u32,         // Max adjust on exit value
    sem_nsems: u32,      // Current number of semaphores in system
    sem_nsets: u32,      // Current number of semaphore sets in system
    sem_largest_id: u32, // Largest semaphore set identifier
}

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy)]
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
pub struct sembuf_t {
    sem_num: u16, // Semaphore number
    sem_op: i16,  // Operation to perform
    sem_flg: i16, // Operation flags
}

#[derive(Debug, Clone)]
struct SemUndoOp {
    semid: SemId,
    sem_num: usize,
    op: i32,
}

struct Semaphore {
    count: i32,
    waiter_zero: HashSet<pid_t>,    // Processes waiting for count = 0
    waiter_acquire: HashSet<pid_t>, // Processes waiting to acquire (count > 0)
    last_pid: pid_t,                // PID of last process that modified this semaphore
}

impl Semaphore {
    fn new(initial_value: i32) -> Self {
        Semaphore {
            count: initial_value,
            waiter_zero: HashSet::new(),
            waiter_acquire: HashSet::new(),
            last_pid: current!().process().pid(),
        }
    }

    /// Checks if an operation can proceed immediately
    /// Returns true if operation can complete, false if process needs to wait
    fn check_op(&mut self, op: i32, pid: pid_t) -> bool {
        match op.cmp(&0) {
            // Positive operation: always allowed (increments semaphore)
            CmpOrdering::Greater => true,

            // Zero operation: wait if count > 0
            CmpOrdering::Equal => {
                if self.count > 0 {
                    self.waiter_zero.insert(pid);
                }
                self.count == 0
            }

            // Negative operation: wait if insufficient count
            CmpOrdering::Less => {
                if self.count + op >= 0 {
                    true
                } else {
                    self.waiter_acquire.insert(pid);
                    false
                }
            }
        }
    }

    /// Executes the semaphore operation and updates waiter sets
    fn do_op(&mut self, op: i32, pid: pid_t) {
        self.count += op;

        // Remove process from wait lists since operation completed
        if op == 0 {
            self.waiter_zero.remove(&pid);
        } else if op < 0 {
            self.waiter_acquire.remove(&pid);
        }
    }

    /// Returns number of processes waiting for count > current value
    fn get_ncnt(&self) -> usize {
        self.waiter_acquire.len()
    }

    /// Returns number of processes waiting for count = 0
    fn get_zcnt(&self) -> usize {
        self.waiter_zero.len()
    }
}

struct SemSet {
    semid: SemId,
    nsems: usize,            // Number of semaphores in this set
    perm: Mutex<ipc_perm_t>, // Permission structure
    sem_otime: AtomicI64,    // Last operation time
    sem_ctime: AtomicI64,    // Creation/modification time

    sems: Mutex<Vec<Semaphore>>,          // The semaphores in this set
    attached_pids: Mutex<HashSet<pid_t>>, // Processes attached to this set
    waiter_queue: Mutex<WaiterQueue>,     // Queue for waiting processes

    is_removed: AtomicBool, // Set to true when semaphore set is removed
    marked_for_removal: AtomicBool, // Set when removal is requested but processes are still attached
}

impl SemSet {
    /// Creates a new semaphore set
    fn new(semid: SemId, key: key_t, nsems: usize, mode: u16) -> Result<Self> {
        info!(
            "New Semset Created: semid: {}, key: {}, nsems: {}, mode: {:o}",
            semid, key, nsems, mode
        );

        // Validate number of semaphores
        if nsems == 0 || nsems > SEMMSL {
            return_errno!(EINVAL, "invalid number of semaphores");
        }

        // Initialize semaphores with value 0
        let sems = (0..nsems).map(|_| Semaphore::new(0)).collect();

        Ok(SemSet {
            semid,
            nsems,
            perm: Mutex::new(ipc_perm_t {
                key,
                uid: 0,
                gid: 0,
                cuid: 0,
                cgid: 0,
                mode,
                pad1: 0,
                seq: 0,
                pad2: 0,
                unused1: 0,
                unused2: 0,
            }),
            sem_otime: AtomicI64::new(0),
            sem_ctime: AtomicI64::new(SemManager::current_time()),
            sems: Mutex::new(sems),
            attached_pids: Mutex::new(HashSet::new()),
            waiter_queue: Mutex::new(WaiterQueue::new()),
            is_removed: AtomicBool::new(false),
            marked_for_removal: AtomicBool::new(false),
        })
    }

    /// Marks semaphore set as removed and wakes all waiting processes
    fn mark_removed_and_wake(&self) {
        self.is_removed.store(true, Ordering::Relaxed);
        let mut waiter_queue = self.waiter_queue.lock();
        waiter_queue.dequeue_and_wake_all();
    }

    fn get_key(&self) -> key_t {
        let perm = self.perm.lock();
        perm.key
    }

    /// Updates permission structure and modification time
    fn set_perm(&self, perm: &ipc_perm_t) {
        let mut current_perm = self.perm.lock();
        *current_perm = *perm;
        self.sem_ctime
            .store(SemManager::current_time(), Ordering::Relaxed);
    }

    fn get_perm(&self) -> ipc_perm_t {
        let perm = self.perm.lock();
        *perm
    }

    /// Records that a process is using this semaphore set
    fn attach_pid(&self, pid: pid_t) {
        let mut pids = self.attached_pids.lock();
        pids.insert(pid);
    }

    /// Removes a process from the attached list
    fn detach_pid(&self, pid: &pid_t) {
        let mut pids = self.attached_pids.lock();
        pids.remove(pid);
    }

    /// Executes a series of semaphore operations
    fn do_semop(&self, sops: &[sembuf_t], mut timeout: Option<Duration>) -> Result<()> {
        let pid = current!().process().pid();
        let waiter = Waiter::new();

        loop {
            // Check if semaphore set was removed
            if self.is_removed.load(Ordering::Relaxed) {
                return_errno!(EIDRM, "semaphore set removed");
            }

            let mut sems = self.sems.lock();
            let mut all_ops_can_proceed = true;

            // First pass: check if all operations can complete
            for sop in sops {
                let sem_num = sop.sem_num as usize;

                // Validate semaphore number
                if sem_num >= self.nsems {
                    return_errno!(EFBIG, "semaphore number out of range");
                }

                // Check if operation can proceed
                if !sems[sem_num].check_op(sop.sem_op as i32, pid) {
                    all_ops_can_proceed = false;

                    // Fail immediately if NOWAIT flag is set
                    let flags = SemFlags::from_bits_truncate(sop.sem_flg as u32);
                    if flags.contains(SemFlags::IPC_NOWAIT) {
                        return_errno!(EAGAIN, "semaphore count not zero");
                    }
                }
            }

            let mut waiter_queue = self.waiter_queue.lock();

            // Execute operations if all can proceed
            if all_ops_can_proceed {
                let mut undo_ops = Vec::new();

                // Apply all operations
                for sop in sops {
                    let sem_num = sop.sem_num as usize;
                    let sem = &mut sems[sem_num];
                    let op = sop.sem_op as i32;

                    sem.do_op(op, pid);
                    sem.last_pid = pid;

                    // Record undo operations if needed
                    let flags = SemFlags::from_bits_truncate(sop.sem_flg as u32);
                    if flags.contains(SemFlags::SEM_UNDO) && op != 0 {
                        undo_ops.push(SemUndoOp {
                            semid: self.semid,
                            sem_num,
                            op: -op,
                        });
                    }
                }

                // Register undo operations with manager
                if !undo_ops.is_empty() {
                    SYSTEM_V_SEM_MANAGER.add_undo_ops(pid, undo_ops);
                }

                // Update operation time and wake waiting processes
                self.sem_otime
                    .store(SemManager::current_time(), Ordering::Relaxed);
                waiter_queue.dequeue_and_wake_all();
                return Ok(());
            }

            // Operations can't proceed - add to wait queue and block
            waiter_queue.reset_and_enqueue(&waiter);
            drop(sems);
            drop(waiter_queue);

            // Wait for notification or timeout
            match waiter.wait_mut(timeout.as_mut()) {
                Ok(()) => continue,
                Err(e) if e.errno() == Errno::ETIMEDOUT => {
                    return_errno!(ETIMEDOUT, "semaphore operation timed out");
                }
                // Handle signal interrupt
                Err(e) if e.errno() == Errno::EINTR => {
                    return_errno!(EINTR, "semaphore operation interrupted by signal");
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Retrieves the current value of a specific semaphore
    fn getval(&self, sem_num: usize) -> Result<i32> {
        if self.is_removed.load(Ordering::Relaxed) {
            return_errno!(EIDRM, "semaphore set removed");
        }
        if sem_num >= self.nsems {
            return_errno!(ERANGE, "semaphore number out of range");
        }
        let sems = self.sems.lock();
        Ok(sems[sem_num].count)
    }

    /// Sets the value of a specific semaphore
    fn setval(&self, sem_num: usize, count: i32) -> Result<()> {
        if self.is_removed.load(Ordering::Relaxed) {
            return_errno!(EIDRM, "semaphore set removed");
        }
        let mut sems = self.sems.lock();

        // Validate parameters
        if sem_num >= self.nsems {
            return_errno!(ERANGE, "semaphore number out of range");
        }
        if count < 0 {
            return_errno!(ERANGE, "semaphore count cannot be negative");
        }
        if count > SEMVMX as i32 {
            return_errno!(ERANGE, "semaphore count exceeds maximum value");
        }

        // Update value and modification time
        sems[sem_num].count = count;
        self.sem_ctime
            .store(SemManager::current_time(), Ordering::Relaxed);
        Ok(())
    }

    /// Returns number of processes waiting for this semaphore's value to increase
    fn get_ncnt(&self, sem_num: usize) -> Result<usize> {
        if self.is_removed.load(Ordering::Relaxed) {
            return_errno!(EIDRM, "semaphore set removed");
        }
        if sem_num >= self.nsems {
            return_errno!(ERANGE, "semaphore number out of range");
        }
        let sems = self.sems.lock();
        Ok(sems[sem_num].get_ncnt())
    }

    /// Returns number of processes waiting for this semaphore's value to reach zero
    fn get_zcnt(&self, sem_num: usize) -> Result<usize> {
        if self.is_removed.load(Ordering::Relaxed) {
            return_errno!(EIDRM, "semaphore set removed");
        }
        if sem_num >= self.nsems {
            return_errno!(ERANGE, "semaphore number out of range");
        }
        let sems = self.sems.lock();
        Ok(sems[sem_num].get_zcnt())
    }

    /// Applies an undo operation to a semaphore
    fn undo_op(&self, sem_num: usize, op: i32) -> Result<()> {
        if self.is_removed.load(Ordering::Relaxed) {
            return_errno!(EIDRM, "semaphore set removed");
        }
        let mut sems = self.sems.lock();
        if sem_num >= self.nsems {
            return_errno!(ERANGE, "semaphore number out of range");
        }

        // Apply the undo operation
        let sem = &mut sems[sem_num];
        sem.count += op;
        Ok(())
    }
}

#[derive(Debug)]
struct SemIdManager {
    used_id: HashSet<SemId>, // Track allocated semaphore IDs
    free_num: u32,           // Number of available IDs
    last_alloc_id: SemId,    // Last allocated ID (for efficient allocation)
}

impl SemIdManager {
    fn new() -> Self {
        SemIdManager {
            used_id: HashSet::new(),
            free_num: SEMMNI as u32,
            last_alloc_id: SEMMNI - 1,
        }
    }

    /// Allocates a new unique semaphore ID
    fn get_new_semid(&mut self) -> Result<SemId> {
        // Check if maximum sets reached
        if self.free_num == 0 {
            return_errno!(ENOSPC, "all possible semaphore IDs have been taken");
        }
        self.free_num -= 1;

        // Find next available ID (wrapping around if necessary)
        let mut id = self.last_alloc_id + 1;
        loop {
            if id == SEMMNI {
                id = 0;
            }
            if !self.used_id.contains(&id) {
                break;
            }
            id += 1;
        }

        self.used_id.insert(id);
        self.last_alloc_id = id;
        Ok(id)
    }

    /// Releases a semaphore ID back to the pool
    fn free_semid(&mut self, shmid: &SemId) -> Result<()> {
        self.free_num += 1;
        self.used_id.remove(shmid);
        Ok(())
    }
}

lazy_static! {
    pub static ref SYSTEM_V_SEM_MANAGER: SemManager = SemManager::new();
}

pub struct SemManager {
    sem_sets: RwLock<HashMap<SemId, Arc<SemSet>>>, // All active semaphore sets
    semid_manager: RwLock<SemIdManager>,           // Manages semaphore ID allocation
    undo_logs: RwLock<HashMap<pid_t, Vec<SemUndoOp>>>, // Tracks undo operations per process
}

impl SemManager {
    fn new() -> Self {
        SemManager {
            sem_sets: RwLock::new(HashMap::new()),
            semid_manager: RwLock::new(SemIdManager::new()),
            undo_logs: RwLock::new(HashMap::new()),
        }
    }

    /// Gets current time for timestamping operations
    fn current_time() -> time_t {
        do_gettimeofday().sec()
    }

    /// Allocates a new semaphore ID through the ID manager
    fn get_new_semid(&self) -> Result<SemId> {
        let mut semid_manager = self.semid_manager.write().unwrap();
        semid_manager.get_new_semid()
    }

    /// Retrieves a semaphore set by ID
    fn get_semset(&self, semid: &SemId) -> Result<Arc<SemSet>> {
        let sem_sets = self.sem_sets.read().unwrap();
        let semset = sem_sets
            .get(semid)
            .ok_or_else(|| errno!(EINVAL, "invalid semid"))?
            .clone();
        Ok(semset)
    }

    /// Releases a semaphore ID
    fn free_semid(&self, semid: &SemId) -> Result<()> {
        let mut semid_manager = self.semid_manager.write().unwrap();
        semid_manager.free_semid(semid)
    }

    /// Adds undo operations for a process
    fn add_undo_ops(&self, pid: pid_t, ops: Vec<SemUndoOp>) {
        let mut undo_logs = self.undo_logs.write().unwrap();
        let entry = undo_logs.entry(pid).or_insert_with(Vec::new);
        entry.extend(ops);
    }

    /// Applies all pending undo operations for a process
    fn perform_undo_ops(&self, pid: pid_t) {
        let mut undo_logs = self.undo_logs.write().unwrap();
        if let Some(ops) = undo_logs.remove(&pid) {
            for op in ops {
                if let Ok(sem_set) = self.get_semset(&op.semid) {
                    let _ = sem_set.undo_op(op.sem_num, op.op);
                }
            }
        }
    }

    /// Returns total number of semaphores across all sets
    fn get_total_semaphores(&self) -> usize {
        let sem_sets = self.sem_sets.read().unwrap();
        sem_sets.values().map(|set| set.nsems).sum()
    }

    /// Returns current number of semaphore sets
    fn get_semset_count(&self) -> usize {
        let sem_sets = self.sem_sets.read().unwrap();
        sem_sets.len()
    }

    /// Returns the largest currently allocated semaphore ID
    fn get_largest_semid(&self) -> SemId {
        let sem_sets = self.sem_sets.read().unwrap();
        sem_sets.keys().max().copied().unwrap_or(0)
    }

    /// Implements semget: creates or retrieves a semaphore set
    pub fn do_semget(&self, key: key_t, nsems: usize, semflg: SemFlags) -> Result<SemId> {
        let mut sem_sets = self.sem_sets.write().unwrap();

        let mode = semflg.bits() as u16 & 0o777;
        let semid = if key == IPC_PRIVATE {
            // Create private semaphore set (always new)
            let semid = self.get_new_semid()?;
            let sem_set = Arc::new(SemSet::new(semid, key, nsems, mode)?);
            sem_sets.insert(sem_set.semid, sem_set);
            semid
        } else {
            // Look for existing set with this key
            let sem_set = sem_sets.values().find(|&set| set.get_key() == key);

            match sem_set {
                Some(set) => {
                    // Handle existing set
                    if semflg.contains(SemFlags::IPC_CREAT) && semflg.contains(SemFlags::IPC_EXCL) {
                        return_errno!(EEXIST, "semaphore set already exists");
                    }
                    if nsems > 0 && nsems != set.nsems {
                        return_errno!(EINVAL, "nsems does not match existing set");
                    }
                    set.semid
                }
                None => {
                    // No existing set - create if requested
                    if !semflg.contains(SemFlags::IPC_CREAT) {
                        return_errno!(ENOENT, "no semaphore set exists for key");
                    }
                    if nsems == 0 || nsems > SEMMSL {
                        return_errno!(EINVAL, "invalid nsems");
                    }

                    // Create new semaphore set
                    let semid = self.get_new_semid()?;
                    let sem_set = Arc::new(SemSet::new(semid, key, nsems, mode)?);
                    sem_sets.insert(sem_set.semid, sem_set);
                    semid
                }
            }
        };
        Ok(semid)
    }

    /// Implements semop: performs semaphore operations
    pub fn do_semop(
        &self,
        semid: SemId,
        sops_ptr: *const sembuf_t,
        nsops: usize,
        mut timeout: Option<Duration>,
    ) -> Result<()> {
        // Validate number of operations
        if nsops == 0 || nsops > SEMOPM {
            return_errno!(E2BIG, "too many operations");
        }

        // Copy operations from user space
        let sops = from_user::make_slice(sops_ptr, nsops)?;
        let pid = current!().process().pid();

        // Get semaphore set and verify it exists
        let sem_set = self.get_semset(&semid)?;

        // Check for race condition (set removed after getting reference)
        if sem_set.is_removed.load(Ordering::Relaxed) {
            return_errno!(EIDRM, "semaphore set removed");
        }

        // Attach process to the set and perform operations
        sem_set.attach_pid(pid);
        let result = sem_set.do_semop(&sops, timeout);

        // Detach process after operations complete
        sem_set.detach_pid(&pid);
        result
    }

    /// Implements semctl: performs control operations on semaphores
    pub fn do_semctl(&self, semid: SemId, semnum: usize, cmd: CmdId, arg: usize) -> Result<usize> {
        info!(
            "do_semctl: semid: {:?}, semnum: {:?}, cmd: {:?}, arg: {:?}",
            semid, semnum, cmd, arg
        );

        // Handle SEM_UNDO command (clear undo operations)
        if cmd == SEM_UNDO {
            let pid = current!().process().pid();
            let mut undo_logs = self.undo_logs.write().unwrap();
            undo_logs.remove(&pid);
            return Ok(0);
        }

        // Handle IPC_RMID (remove semaphore set)
        if cmd == IPC_RMID {
            let sem_set = self.get_semset(&semid)?;
            // Mark as removed and wake waiting processes
            sem_set.mark_removed_and_wake();

            sem_set.marked_for_removal.store(true, Ordering::Relaxed);

            // Remove immediately if no processes are attached
            let is_empty = {
                let pids = sem_set.attached_pids.lock();
                pids.is_empty()
            };
            if is_empty {
                self.free_semid(&semid)?;
                let mut sem_sets = self.sem_sets.write().unwrap();
                sem_sets.remove(&semid);
            }
            return Ok(0);
        }

        // Handle commands that don't require a specific semaphore set
        match cmd {
            IPC_INFO => {
                // Fill system semaphore limits
                let info_ptr = arg as *mut seminfo_t;
                let info = unsafe {
                    info_ptr
                        .as_mut()
                        .ok_or_else(|| errno!(EFAULT, "invalid pointer"))?
                };

                *info = seminfo_t {
                    semmap: 0,
                    semmni: SEMMNI,
                    semmns: SEMMNS as u32,
                    semmnu: 0,
                    semmsl: SEMMSL as u32,
                    semopm: SEMOPM as u32,
                    semume: 0,
                    semusz: 0,
                    semvmx: SEMVMX as u32,
                    semaem: 0,
                };

                return Ok(SEMMNI as usize);
            }

            SEM_INFO => {
                // Fill extended semaphore information
                let info_ptr = arg as *mut seminfo_ext_t;
                let info = unsafe {
                    info_ptr
                        .as_mut()
                        .ok_or_else(|| errno!(EFAULT, "invalid pointer"))?
                };

                // Base limit information
                let base_info = seminfo_t {
                    semmap: 0,
                    semmni: SEMMNI,
                    semmns: SEMMNS as u32,
                    semmnu: 0,
                    semmsl: SEMMSL as u32,
                    semopm: SEMOPM as u32,
                    semume: 0,
                    semusz: 0,
                    semvmx: SEMVMX as u32,
                    semaem: 0,
                };

                // Current system status
                *info = seminfo_ext_t {
                    sem_info: base_info,
                    semusz: 0,
                    semaem: 0,
                    sem_nsems: self.get_total_semaphores() as u32,
                    sem_nsets: self.get_semset_count() as u32,
                    sem_largest_id: self.get_largest_semid(),
                };

                return Ok(self.get_largest_semid() as usize);
            }

            _ => {} // Other commands require a semaphore set
        }

        // Get semaphore set for remaining commands
        let sem_set = self.get_semset(&semid)?;

        // Check if set was removed
        if sem_set.is_removed.load(Ordering::Relaxed) {
            return_errno!(EIDRM, "semaphore set removed");
        }

        // Handle set-specific commands
        match cmd {
            IPC_SET => {
                // Update permission structure
                let perm = arg as *const ipc_perm_t;
                let perm = unsafe {
                    perm.as_ref()
                        .ok_or_else(|| errno!(EFAULT, "invalid perm"))?
                };
                sem_set.set_perm(perm);
                Ok(0)
            }
            IPC_STAT => {
                // Retrieve status information
                let buf_ptr = arg as *mut semids_t;
                let buf = unsafe {
                    buf_ptr
                        .as_mut()
                        .ok_or_else(|| errno!(EFAULT, "invalid buf"))?
                };
                *buf = semids_t {
                    sem_perm: sem_set.get_perm(),
                    sem_otime: sem_set.sem_otime.load(Ordering::Relaxed),
                    sem_otime_high: 0,
                    sem_ctime: sem_set.sem_ctime.load(Ordering::Relaxed),
                    sem_ctime_high: 0,
                    sem_nsems: sem_set.nsems as u64,
                    unused1: 0,
                    unused2: 0,
                };
                Ok(0)
            }
            SEM_GETPID => {
                // Get PID of last operation
                if semnum >= sem_set.nsems {
                    return_errno!(ERANGE, "semaphore number out of range");
                }
                let sems = sem_set.sems.lock();
                Ok(sems[semnum].last_pid as usize)
            }
            SEM_GETVAL => {
                // Get current semaphore value
                let value = sem_set.getval(semnum)?;
                Ok(value as usize)
            }
            SEM_GETALL => {
                // Get all semaphore values in set
                let vals_ptr = arg as *mut u16;
                if vals_ptr.is_null() {
                    return_errno!(EFAULT, "null pointer");
                }

                let vals = from_user::make_mut_slice(vals_ptr, sem_set.nsems)?;
                for i in 0..sem_set.nsems {
                    vals[i] = sem_set.getval(i)? as u16;
                }
                Ok(0)
            }
            SEM_GETNCNT => {
                // Get count of processes waiting for higher value
                let ncnt = sem_set.get_ncnt(semnum)?;
                Ok(ncnt)
            }
            SEM_GETZCNT => {
                // Get count of processes waiting for zero
                let zcnt = sem_set.get_zcnt(semnum)?;
                Ok(zcnt)
            }
            SEM_SETVAL => {
                // Set individual semaphore value
                let value = arg as i32;
                sem_set.setval(semnum, value)?;
                Ok(0)
            }
            SEM_SETALL => {
                // Set all semaphore values in set
                let vals_ptr = arg as *const u16;
                if vals_ptr.is_null() {
                    return_errno!(EFAULT, "null pointer");
                }

                let vals = from_user::make_slice(vals_ptr, sem_set.nsems)?;
                for i in 0..sem_set.nsems {
                    let value = vals[i] as i32;
                    sem_set.setval(i, value)?;
                }
                Ok(0)
            }
            SEM_STAT => {
                // Get status by semid
                let buf_ptr = arg as *mut semids_t;
                let buf = unsafe {
                    buf_ptr
                        .as_mut()
                        .ok_or_else(|| errno!(EFAULT, "invalid buf"))?
                };

                *buf = semids_t {
                    sem_perm: sem_set.get_perm(),
                    sem_otime: sem_set.sem_otime.load(Ordering::Relaxed),
                    sem_otime_high: 0,
                    sem_ctime: sem_set.sem_ctime.load(Ordering::Relaxed),
                    sem_ctime_high: 0,
                    sem_nsems: sem_set.nsems as u64,
                    unused1: 0,
                    unused2: 0,
                };
                Ok(sem_set.semid as usize)
            }
            _ => return_errno!(EINVAL, "unsupported command"),
        }
    }

    /// Cleans up semaphore resources when a process exits
    pub fn detach_sem_when_process_exit(&self, thread: &ThreadRef) {
        let pid = thread.process().pid();
        // Apply any pending undo operations
        self.perform_undo_ops(pid);

        // Detach process from all semaphore sets
        let mut sem_sets = self.sem_sets.write().unwrap();
        for (_, sem_set) in sem_sets.iter_mut() {
            sem_set.detach_pid(&pid);
        }
    }

    /// Cleans up all semaphore resources on system exit
    pub fn clean_when_libos_exit(&self) {
        let mut sem_sets = self.sem_sets.write().unwrap();
        for (semid, _) in sem_sets.drain() {
            self.free_semid(&semid);
        }
    }
}
