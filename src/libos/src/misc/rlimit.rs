use super::*;
use process::pid_t;

#[derive(Debug, Copy, Clone)]
pub struct ResourceLimits {
    rlimits: [rlimit_t; RLIMIT_COUNT],
}
pub type ResourceLimitsRef = Arc<SgxMutex<ResourceLimits>>;

impl ResourceLimits {
    pub fn get(&self, resource: resource_t) -> &rlimit_t {
        &self.rlimits[resource as usize]
    }

    pub fn get_mut(&mut self, resource: resource_t) -> &mut rlimit_t {
        &mut self.rlimits[resource as usize]
    }
}

impl Default for ResourceLimits {
    fn default() -> ResourceLimits {
        // TODO: set appropriate limits for resources
        let mut rlimits = ResourceLimits {
            rlimits: [Default::default(); RLIMIT_COUNT],
        };
        rlimits
    }
}

#[derive(Debug, Copy, Clone)]
#[allow(non_camel_case_types)]
pub struct rlimit_t {
    cur: u64,
    max: u64,
}

impl Default for rlimit_t {
    fn default() -> rlimit_t {
        rlimit_t {
            cur: u64::max_value(),
            max: u64::max_value(),
        }
    }
}

#[derive(Debug, Copy, Clone)]
#[allow(non_camel_case_types)]
pub enum resource_t {
    RLIMIT_CPU = 0,
    RLIMIT_FSIZE = 1,
    RLIMIT_DATA = 2,
    RLIMIT_STACK = 3,
    RLIMIT_CORE = 4,
    RLIMIT_RSS = 5,
    RLIMIT_NPROC = 6,
    RLIMIT_NOFILE = 7,
    RLIMIT_MEMLOCK = 8,
    RLIMIT_AS = 9,
    RLIMIT_LOCKS = 10,
    RLIMIT_SIGPENDING = 11,
    RLIMIT_MSGQUEUE = 12,
    RLIMIT_NICE = 13,
    RLIMIT_RTPRIO = 14,
}
const RLIMIT_COUNT: usize = 15;

impl resource_t {
    pub fn from_u32(bits: u32) -> Result<resource_t> {
        match bits {
            0 => Ok(resource_t::RLIMIT_CPU),
            1 => Ok(resource_t::RLIMIT_FSIZE),
            2 => Ok(resource_t::RLIMIT_DATA),
            3 => Ok(resource_t::RLIMIT_STACK),
            4 => Ok(resource_t::RLIMIT_CORE),
            5 => Ok(resource_t::RLIMIT_RSS),
            6 => Ok(resource_t::RLIMIT_NPROC),
            7 => Ok(resource_t::RLIMIT_NOFILE),
            8 => Ok(resource_t::RLIMIT_MEMLOCK),
            9 => Ok(resource_t::RLIMIT_AS),
            10 => Ok(resource_t::RLIMIT_LOCKS),
            11 => Ok(resource_t::RLIMIT_SIGPENDING),
            12 => Ok(resource_t::RLIMIT_MSGQUEUE),
            13 => Ok(resource_t::RLIMIT_NICE),
            14 => Ok(resource_t::RLIMIT_RTPRIO),
            _ => return_errno!(EINVAL, "invalid resource"),
        }
    }
}

pub fn do_prlimit(
    pid: pid_t,
    resource: resource_t,
    new_limit: Option<&rlimit_t>,
    old_limit: Option<&mut rlimit_t>,
) -> Result<()> {
    let process_ref = if pid == 0 {
        process::get_current()
    } else {
        process::get(pid).cause_err(|_| errno!(ESRCH, "invalid pid"))?
    };
    let mut process = process_ref.lock().unwrap();
    let rlimits_ref = process.get_rlimits();
    let mut rlimits = rlimits_ref.lock().unwrap();
    if let Some(old_limit) = old_limit {
        *old_limit = *rlimits.get(resource)
    }
    if let Some(new_limit) = new_limit {
        *rlimits.get_mut(resource) = *new_limit;
    }
    Ok(())
}

pub fn do_getrlimit(resource: resource_t, old_limit: &mut rlimit_t) -> Result<()> {
    do_prlimit(0 as pid_t, resource, None, Some(old_limit))
}

pub fn do_setrlimit(resource: resource_t, new_limit: &rlimit_t) -> Result<()> {
    do_prlimit(0 as pid_t, resource, Some(new_limit), None)
}
