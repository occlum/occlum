use super::*;

mod uname;
mod rlimit;

pub use self::uname::{utsname_t, do_uname};
pub use self::rlimit::{rlimit_t, resource_t, ResourceLimits, ResourceLimitsRef, do_prlimit};
