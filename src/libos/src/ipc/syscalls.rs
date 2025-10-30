use super::*;

use crate::time::timespec_t;
use std::time::Duration;
use util::mem_util::from_user;

use super::sem::{sembuf_t, semids_t, SemFlags, SemId, SYSTEM_V_SEM_MANAGER};
use super::shm::{shmids_t, CmdId, ShmFlags, ShmId, SYSTEM_V_SHM_MANAGER};

pub fn do_shmget(key: key_t, size: size_t, shmflg: i32) -> Result<isize> {
    let shmflg =
        ShmFlags::from_bits(shmflg as u32).ok_or_else(|| errno!(EINVAL, "invalid flags"))?;
    let shmid = SYSTEM_V_SHM_MANAGER.do_shmget(key, size, shmflg)?;
    Ok(shmid as isize)
}

pub fn do_shmat(shmid: i32, shmaddr: usize, shmflg: i32) -> Result<isize> {
    let shmflg =
        ShmFlags::from_bits(shmflg as u32).ok_or_else(|| errno!(EINVAL, "invalid flags"))?;
    let addr = SYSTEM_V_SHM_MANAGER.do_shmat(shmid as ShmId, shmaddr, shmflg)?;
    Ok(addr as isize)
}

pub fn do_shmdt(shmaddr: usize) -> Result<isize> {
    SYSTEM_V_SHM_MANAGER.do_shmdt(shmaddr)?;
    Ok(0)
}

pub fn do_shmctl(shmid: i32, cmd: i32, buf_u: *mut shmids_t) -> Result<isize> {
    let buf = if !buf_u.is_null() {
        from_user::check_mut_ptr(buf_u)?;
        let mut buf = unsafe { &mut *buf_u };
        Some(buf)
    } else {
        None
    };
    SYSTEM_V_SHM_MANAGER.do_shmctl(shmid as ShmId, cmd as CmdId, buf)?;
    Ok(0)
}

pub fn do_semget(key: key_t, nsems: i32, semflg: i32) -> Result<isize> {
    let semflg =
        SemFlags::from_bits(semflg as u32).ok_or_else(|| errno!(EINVAL, "invalid flags"))?;
    let semid = SYSTEM_V_SEM_MANAGER.do_semget(key, nsems as usize, semflg)?;
    Ok(semid as isize)
}

pub fn do_semctl(semid: i32, semnum: i32, cmd: i32, arg: usize) -> Result<isize> {
    let cmd = {
        if cmd >= 256 {
            cmd - 256
        } else {
            cmd
        }
    };

    let ret = SYSTEM_V_SEM_MANAGER.do_semctl(semid as SemId, semnum as usize, cmd as CmdId, arg)?;
    Ok(ret as isize)
}

pub fn do_semop(semid: i32, sops_ptr: *const sembuf_t, nsops: usize) -> Result<isize> {
    if nsops == 0 {
        return Ok(0);
    }
    let ret = SYSTEM_V_SEM_MANAGER.do_semop(semid as SemId, sops_ptr, nsops, None)?;
    Ok(9)
}

pub fn do_semtimedop(
    semid: i32,
    sops_ptr: *const sembuf_t,
    nsops: usize,
    timeout_ptr: *const timespec_t,
) -> Result<isize> {
    if nsops == 0 {
        return Ok(0);
    }

    let timeout: Option<Duration> = {
        if timeout_ptr.is_null() {
            None
        } else {
            let timeout = timespec_t::from_raw_ptr(timeout_ptr)?;
            Some(timeout.as_duration())
        }
    };

    SYSTEM_V_SEM_MANAGER.do_semop(semid as SemId, sops_ptr, nsops, timeout)?;
    Ok(0)
}
