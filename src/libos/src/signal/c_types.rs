#![allow(non_camel_case_types)]

use std::fmt;

use super::SigNum;
use crate::prelude::*;
use crate::syscall::CpuContext;
use crate::time::clock_t;

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct sigaction_t {
    pub handler: *const c_void,
    pub flags: u32,
    pub restorer: *const c_void,
    pub mask: sigset_t,
}

pub type sigset_t = u64;

#[derive(Clone, Copy)]
#[repr(C)]
pub union sigval_t {
    sigval_int: i32,
    sigval_ptr: *mut c_void,
}

impl fmt::Debug for sigval_t {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "sigval_t = {{ {:?} or {:?} }}",
            unsafe { self.sigval_int },
            unsafe { self.sigval_ptr }
        )
    }
}

impl From<i32> for sigval_t {
    fn from(val: i32) -> sigval_t {
        sigval_t { sigval_int: val }
    }
}

impl<T> From<*mut T> for sigval_t {
    fn from(ptr: *mut T) -> sigval_t {
        sigval_t {
            sigval_ptr: ptr as *mut c_void,
        }
    }
}

impl<T> From<*const T> for sigval_t {
    fn from(ptr: *const T) -> sigval_t {
        sigval_t {
            sigval_ptr: ptr as *const c_void as *mut c_void,
        }
    }
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct siginfo_t {
    pub si_signo: i32,
    pub si_errno: i32,
    pub si_code: i32,
    _padding: i32,
    fields: siginfo_fields_t,
}

#[derive(Clone, Copy)]
#[repr(C)]
union siginfo_fields_t {
    bytes: [u8; 128 - std::mem::size_of::<i32>() * 4],
    common: siginfo_common_t,
    sigfault: siginfo_sigfault_t,
    //sigpoll: siginfo_poll_t,
    //sigsys: siginfo_sys_t,
}

#[derive(Clone, Copy)]
#[repr(C)]
union siginfo_common_t {
    first: siginfo_common_first_t,
    second: siginfo_common_second_t,
}

#[derive(Clone, Copy)]
#[repr(C)]
union siginfo_common_first_t {
    piduid: siginfo_piduid_t,
    timer: siginfo_timer_t,
}

#[derive(Clone, Copy)]
#[repr(C)]
struct siginfo_piduid_t {
    pid: pid_t,
    uid: uid_t,
}

#[derive(Clone, Copy)]
#[repr(C)]
struct siginfo_timer_t {
    timerid: i32,
    overrun: i32,
}

#[derive(Clone, Copy)]
#[repr(C)]
union siginfo_common_second_t {
    value: sigval_t,
    sigchild: siginfo_sigchild_t,
}

#[derive(Clone, Copy)]
#[repr(C)]
union siginfo_sigchild_t {
    status: i32,
    utime: clock_t,
    stime: clock_t,
}

#[derive(Clone, Copy)]
#[repr(C)]
struct siginfo_sigfault_t {
    addr: *const c_void,
    addr_lsb: i16,
    first: siginfo_sigfault_first_t,
}

#[derive(Clone, Copy)]
#[repr(C)]
union siginfo_sigfault_first_t {
    addr_bnd: siginfo_addr_bnd_t,
    pkey: u32,
}

#[derive(Clone, Copy)]
#[repr(C)]
union siginfo_addr_bnd_t {
    lower: *const c_void,
    upper: *const c_void,
}

impl siginfo_t {
    pub fn new(num: SigNum, code: i32) -> Self {
        let zero_fields = siginfo_fields_t {
            bytes: [0_u8; std::mem::size_of::<siginfo_fields_t>()],
        };
        Self {
            si_signo: num.as_u8() as i32,
            si_code: code,
            si_errno: 0,
            _padding: 0,
            fields: zero_fields,
        }
    }
}

// Use macros to implement the getter and setter functions of siginfo_t. These getters
// and setters help the user to access the values embedded inside the many unions of
// siginfo_t.
macro_rules! impl_siginfo_getters_setters {
    ( $( $getter:ident, $setter:ident : $val_type:ty => $( $path:ident ).* ),+,  ) => {
        $(
            pub fn $getter(&self) -> $val_type {
                unsafe {
                    self.$($path).*
                }
            }

            pub fn $setter(&mut self, new_val: $val_type) {
                unsafe {
                    self.$($path).* = new_val;
                }
            }
        )*
    }
}

impl siginfo_t {
    impl_siginfo_getters_setters! {
        // Format:
        //  getter_name, setter_name : field_type => path_to_field
        si_pid, set_si_pid : pid_t => fields.common.first.piduid.pid,
        si_uid, set_si_uid : uid_t => fields.common.first.piduid.uid,
        si_status, set_si_satus : i32 => fields.common.second.sigchild.status,
        si_utime, set_si_utime : clock_t => fields.common.second.sigchild.utime,
        si_stime, set_si_stime : clock_t => fields.common.second.sigchild.stime,
        si_value, set_si_value : sigval_t => fields.common.second.value,
        si_addr, set_si_addr : *const c_void => fields.sigfault.addr,
        si_addr_lsb, set_si_addr_lsb : i16 => fields.sigfault.addr_lsb,
        si_lower, set_si_lower : *const c_void => fields.sigfault.first.addr_bnd.lower,
        si_upper, set_si_upper : *const c_void => fields.sigfault.first.addr_bnd.upper,
        si_pkey, set_si_pkey : u32 => fields.sigfault.first.pkey,
        si_timerid, set_si_timerid : i32 => fields.common.first.timer.timerid,
        si_overrune, set_si_overrune : i32 => fields.common.first.timer.overrun,
    }
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct ucontext_t {
    pub uc_flags: u64,
    pub uc_link: *mut ucontext_t,
    pub uc_stack: stack_t,
    pub uc_mcontext: mcontext_t,
    pub uc_sigmask: sigset_t,
    pub fpregs: [u8; 64 * 8], //fxsave structure
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct sigaltstack_t {
    pub ss_sp: *mut c_void,
    pub ss_flags: i32,
    pub ss_size: usize,
}

pub type stack_t = sigaltstack_t;

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct mcontext_t {
    pub inner: CpuContext,
    // TODO: the fields should be csgsfs, err, trapno, oldmask, and cr2
    _unused0: [u64; 5],
    // TODO: this field should be `fpregs: fpregset_t,`
    _unused1: usize,
    _reserved: [u64; 8],
}

/// Special values for the user-given signal handlers
pub const SIG_ERR: *const c_void = -1_i64 as *const c_void;
pub const SIG_DFL: *const c_void = 0_i64 as *const c_void;
pub const SIG_IGN: *const c_void = 1_i64 as *const c_void;

pub const SI_ASYNCNL: i32 = -60;
pub const SI_TKILL: i32 = -6;
pub const SI_SIGIO: i32 = -5;
pub const SI_ASYNCIO: i32 = -4;
pub const SI_MESGQ: i32 = -3;
pub const SI_TIMER: i32 = -2;
pub const SI_QUEUE: i32 = -1;
pub const SI_USER: i32 = 0;
pub const SI_KERNEL: i32 = 128;

pub const FPE_INTDIV: i32 = 1;
pub const FPE_INTOVF: i32 = 2;
pub const FPE_FLTDIV: i32 = 3;
pub const FPE_FLTOVF: i32 = 4;
pub const FPE_FLTUND: i32 = 5;
pub const FPE_FLTRES: i32 = 6;
pub const FPE_FLTINV: i32 = 7;
pub const FPE_FLTSUB: i32 = 8;

pub const ILL_ILLOPC: i32 = 1;
pub const ILL_ILLOPN: i32 = 2;
pub const ILL_ILLADR: i32 = 3;
pub const ILL_ILLTRP: i32 = 4;
pub const ILL_PRVOPC: i32 = 5;
pub const ILL_PRVREG: i32 = 6;
pub const ILL_COPROC: i32 = 7;
pub const ILL_BADSTK: i32 = 8;

pub const SEGV_MAPERR: i32 = 1;
pub const SEGV_ACCERR: i32 = 2;
pub const SEGV_BNDERR: i32 = 3;
pub const SEGV_PKUERR: i32 = 4;

pub const BUS_ADRALN: i32 = 1;
pub const BUS_ADRERR: i32 = 2;
pub const BUS_OBJERR: i32 = 3;
pub const BUS_MCEERR_AR: i32 = 4;
pub const BUS_MCEERR_AO: i32 = 5;

pub const CLD_EXITED: i32 = 1;
pub const CLD_KILLED: i32 = 2;
pub const CLD_DUMPED: i32 = 3;
pub const CLD_TRAPPED: i32 = 4;
pub const CLD_STOPPED: i32 = 5;
pub const CLD_CONTINUED: i32 = 6;
