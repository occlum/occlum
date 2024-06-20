use crate::error::*;
use crate::time::timeval_t;
use std::convert::TryFrom;

#[repr(C)]
#[derive(Default)]
pub struct Rusage {
    ru_utime: timeval_t,
    ru_stime: timeval_t,

    // For x86-64, `long int` is 64-bit long. Just use i64 here
    ru_maxrss: i64,   // maximum resident set size
    ru_ixrss: i64,    // integral shared memory size
    ru_idrss: i64,    // integral unshared data size
    ru_isrss: i64,    // integral unshared stack size
    ru_minflt: i64,   // page reclaims (soft page faults)
    ru_majflt: i64,   // page faults (hard page faults)
    ru_nswap: i64,    // swaps
    ru_inblock: i64,  // block input operations
    ru_oublock: i64,  // block output operations
    ru_msgsnd: i64,   // IPC messages sent
    ru_msgrcv: i64,   // IPC messages received
    ru_nsignals: i64, // signals received
    ru_nvcsw: i64,    // voluntary context switches
    ru_nivcsw: i64,   // involuntary context switches
}

#[derive(Debug)]
#[repr(i32)]
pub enum RusageWho {
    RUSAGE_SELF = 0,
    RUSAGE_CHILDREN = -1,
    RUSAGE_THREAD = 1,
}

impl TryFrom<i32> for RusageWho {
    type Error = Error;

    fn try_from(value: i32) -> Result<Self> {
        match value {
            0 => Ok(RusageWho::RUSAGE_SELF),
            -1 => Ok(RusageWho::RUSAGE_CHILDREN),
            1 => Ok(RusageWho::RUSAGE_THREAD),
            _ => return_errno!(EINVAL, "invalid rusage who"),
        }
    }
}

pub fn do_getrusage(who: RusageWho, rusage: &mut Rusage) -> Result<()> {
    debug!("getrusage who: {:?}", who);
    let mut zero_rusage = Rusage::default();

    core::mem::swap(rusage, &mut zero_rusage);
    Ok(())
}
