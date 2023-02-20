use super::*;
use process::pid_t;

#[derive(Debug, Copy, Clone)]
pub struct ResourceLimits {
    rlimits: [rlimit_t; RLIMIT_COUNT],
}

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
        // Get memory space limit from Occlum.json
        let cfg_heap_size: u64 = config::LIBOS_CONFIG.process.default_heap_size as u64;
        let cfg_stack_size: u64 = config::LIBOS_CONFIG.process.default_stack_size as u64;
        let cfg_user_space_size: u64 =
            config::LIBOS_CONFIG.resource_limits.user_space_max_size as u64;

        let stack_size = rlimit_t::new(cfg_stack_size);

        // Data segment consists of three parts: initialized data, uninitialized data, and heap.
        // Here we just approximatively consider this equal to the size of heap size.
        let data_size = rlimit_t::new(cfg_heap_size);
        // Address space can be approximatively considered equal to user space.
        let address_space = rlimit_t::new(cfg_user_space_size);

        // Set init open files limit to 1024 which is default value for Ubuntu
        let open_files = rlimit_t::new(1024);

        let mut rlimits = ResourceLimits {
            rlimits: [Default::default(); RLIMIT_COUNT],
        };
        *rlimits.get_mut(resource_t::RLIMIT_DATA) = data_size;
        *rlimits.get_mut(resource_t::RLIMIT_STACK) = stack_size;
        *rlimits.get_mut(resource_t::RLIMIT_AS) = address_space;
        *rlimits.get_mut(resource_t::RLIMIT_NOFILE) = open_files;

        rlimits
    }
}

#[derive(Debug, Copy, Clone)]
#[allow(non_camel_case_types)]
pub struct rlimit_t {
    cur: u64,
    max: u64,
}

impl rlimit_t {
    fn new(cur: u64) -> rlimit_t {
        rlimit_t {
            cur: cur,
            max: u64::max_value(),
        }
    }

    pub fn get_cur(&self) -> u64 {
        self.cur
    }

    pub fn get_max(&self) -> u64 {
        self.max
    }
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
    RLIMIT_RTTIME = 15,
}
const RLIMIT_COUNT: usize = 16;

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
            15 => Ok(resource_t::RLIMIT_RTTIME),
            _ => return_errno!(EINVAL, "invalid resource"),
        }
    }
}

/// Get or set resource limits.
///
/// The man page suggests that this system call works on a per-process basis
/// and the input argument pid can only be process ID, not thread ID. This
/// (unnecessary) restriction is lifted by our implementation. Nevertheless,
/// since the rlimits object is shared between threads in a process, the
/// semantic of limiting resource usage on a per-process basis is preserved.
///
/// Limitation: Current implementation only takes effect on child processes.
pub fn do_prlimit(
    pid: pid_t,
    resource: resource_t,
    new_limit: Option<&rlimit_t>,
    old_limit: Option<&mut rlimit_t>,
) -> Result<()> {
    let process = if pid == 0 {
        current!()
    } else {
        process::table::get_thread(pid).cause_err(|_| errno!(ESRCH, "invalid pid"))?
    };
    let mut rlimits = process.rlimits().lock().unwrap();
    if let Some(old_limit) = old_limit {
        *old_limit = *rlimits.get(resource)
    }
    if let Some(new_limit) = new_limit {
        if new_limit.get_cur() > new_limit.get_max() {
            return_errno!(EINVAL, "soft limit is greater than hard limit");
        }

        let mut soft_rlimit_stack_size = rlimits.get(resource_t::RLIMIT_STACK).get_cur();
        let mut soft_rlimit_data_size = rlimits.get(resource_t::RLIMIT_DATA).get_cur();
        let mut soft_rlimit_address_space_size = rlimits.get(resource_t::RLIMIT_AS).get_cur();
        match resource {
            resource_t::RLIMIT_DATA => {
                soft_rlimit_data_size = new_limit.get_cur();
            }
            resource_t::RLIMIT_STACK => {
                soft_rlimit_stack_size = new_limit.get_cur();
            }
            resource_t::RLIMIT_AS => {
                soft_rlimit_address_space_size = new_limit.get_cur();
            }
            _ => (),
        }

        let soft_data_and_stack_size = soft_rlimit_data_size
            .checked_add(soft_rlimit_stack_size)
            .ok_or_else(|| errno!(EOVERFLOW, "memory size overflow"))?;

        // Mmap space size can't be zero at least.
        if soft_rlimit_address_space_size <= soft_data_and_stack_size {
            return_errno!(EINVAL, "RLIMIT_AS size is too small");
        }

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
