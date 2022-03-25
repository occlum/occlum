use super::*;

mod shm;
mod syscalls;

pub use self::shm::{key_t, shmids_t, SHM_MANAGER};
pub use self::syscalls::{do_shmat, do_shmctl, do_shmdt, do_shmget};
