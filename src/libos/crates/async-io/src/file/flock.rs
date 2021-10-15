/// File POSIX advisory lock
use crate::prelude::*;
use libc::{off_t, pid_t};

/// C struct for a lock
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct flock_c {
    pub l_type: u16,
    pub l_whence: u16,
    pub l_start: off_t,
    pub l_len: off_t,
    pub l_pid: pid_t,
}

impl flock_c {
    pub fn copy_from_safe(&mut self, lock: &Flock) {
        self.l_type = lock.l_type as u16;
        self.l_whence = lock.l_whence as u16;
        self.l_start = lock.l_start;
        self.l_len = lock.l_len;
        self.l_pid = lock.l_pid;
    }
}

/// Type safe representation of flock
#[derive(Debug, Copy, Clone)]
pub struct Flock {
    pub l_type: FlockType,
    pub l_whence: FlockWhence,
    pub l_start: off_t,
    pub l_len: off_t,
    pub l_pid: pid_t,
}

impl Flock {
    pub fn from_c(flock_c: &flock_c) -> Result<Self> {
        let l_type = FlockType::from_u16(flock_c.l_type)?;
        let l_whence = FlockWhence::from_u16(flock_c.l_whence)?;
        Ok(Self {
            l_type: l_type,
            l_whence: l_whence,
            l_start: flock_c.l_start,
            l_len: flock_c.l_len,
            l_pid: flock_c.l_pid,
        })
    }
}

#[allow(non_camel_case_types)]
#[derive(Debug, Copy, Clone)]
#[repr(u16)]
pub enum FlockType {
    F_RDLCK = 0,
    F_WRLCK = 1,
    F_UNLCK = 2,
}

impl FlockType {
    pub fn from_u16(_type: u16) -> Result<Self> {
        Ok(match _type {
            0 => FlockType::F_RDLCK,
            1 => FlockType::F_WRLCK,
            2 => FlockType::F_UNLCK,
            _ => return_errno!(EINVAL, "invalid flock type"),
        })
    }
}

#[allow(non_camel_case_types)]
#[derive(Debug, Copy, Clone)]
#[repr(u16)]
pub enum FlockWhence {
    SEEK_SET = 0,
    SEEK_CUR = 1,
    SEEK_END = 2,
}

impl FlockWhence {
    pub fn from_u16(whence: u16) -> Result<Self> {
        Ok(match whence {
            0 => FlockWhence::SEEK_SET,
            1 => FlockWhence::SEEK_CUR,
            2 => FlockWhence::SEEK_END,
            _ => return_errno!(EINVAL, "Invalid whence"),
        })
    }
}
