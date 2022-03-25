use super::*;

use util::mem_util::from_user;

use super::shm::{shmids_t, CmdId, ShmFlags, ShmId, SHM_MANAGER};

pub fn do_shmget(key: key_t, size: size_t, shmflg: i32) -> Result<isize> {
    let shmflg =
        ShmFlags::from_bits(shmflg as u32).ok_or_else(|| errno!(EINVAL, "invalid flags"))?;
    let shmid = SHM_MANAGER.do_shmget(key, size, shmflg)?;
    Ok(shmid as isize)
}

pub fn do_shmat(shmid: i32, shmaddr: usize, shmflg: i32) -> Result<isize> {
    let shmflg =
        ShmFlags::from_bits(shmflg as u32).ok_or_else(|| errno!(EINVAL, "invalid flags"))?;
    let addr = SHM_MANAGER.do_shmat(shmid as ShmId, shmaddr, shmflg)?;
    Ok(addr as isize)
}

pub fn do_shmdt(shmaddr: usize) -> Result<isize> {
    SHM_MANAGER.do_shmdt(shmaddr)?;
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
    SHM_MANAGER.do_shmctl(shmid as ShmId, cmd as CmdId, buf)?;
    Ok(0)
}
