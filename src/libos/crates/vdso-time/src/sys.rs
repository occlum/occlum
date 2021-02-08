use libc::clockid_t;
use std::sync::atomic::{AtomicU32, Ordering};

pub const PAGE_SIZE: u64 = 4096;

pub const CLOCK_TAI: usize = 11;
pub const VDSO_BASES: usize = CLOCK_TAI + 1;

pub const NSEC_PER_USEC: u64 = 1000;
pub const NSEC_PER_SEC: u64 = 1000000000;

/// The timers is divided in 3 sets (HRES, COARSE, RAW),
/// CS_HRES_COARSE refers to the first two and CS_RAW to the third.
#[derive(Debug, Copy, Clone)]
#[allow(non_camel_case_types)]
pub enum ClockSource {
    CS_HRES_COARSE = 0,
    CS_RAW = 1,
}

#[derive(Debug, Copy, Clone)]
#[allow(non_camel_case_types)]
pub enum vdso_clock_mode {
    VDSO_CLOCKMODE_NONE = 0,
}

// libc::timezone is enum {}, need re-define timezone
#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct timezone {
    pub tz_minuteswest: i32, /* Minutes west of GMT.  */
    pub tz_dsttime: i32,     /* Nonzero if DST is ever in effect.  */
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct arch_vdso_data {}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct vdso_timestamp {
    pub sec: u64,
    pub nsec: u64,
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct timens_offset {
    pub sec: i64,
    pub nsec: u64,
}

pub trait VdsoData {
    fn vdso_timestamp(&self, clockid: clockid_t) -> &vdso_timestamp;
    fn seq(&self) -> u32;
    fn clock_mode(&self) -> i32;
    fn cycle_last(&self) -> u64;
    fn mask(&self) -> u64;
    fn mult(&self) -> u32;
    fn shift(&self) -> u32;
    fn tz_minuteswest(&self) -> i32;
    fn tz_dsttime(&self) -> i32;
    fn hrtimer_res(&self) -> u32;
}

pub enum VdsoDataPtr {
    V5_9(*const vdso_data_v5_9),
}

#[repr(C)]
pub struct vdso_data_v5_9 {
    pub seq: AtomicU32,

    pub clock_mode: i32,
    pub cycle_last: u64,
    pub mask: u64,
    pub mult: u32,
    pub shift: u32,

    pub union_1: vdso_data_v5_9_union_1,

    pub tz_minuteswest: i32,
    pub tz_dsttime: i32,
    pub hrtimer_res: u32,
    pub __unused: u32,

    pub arch_data: arch_vdso_data,
}

impl VdsoData for vdso_data_v5_9 {
    #[inline]
    fn vdso_timestamp(&self, clockid: clockid_t) -> &vdso_timestamp {
        unsafe { &self.union_1.basetime[clockid as usize] }
    }

    #[inline]
    fn seq(&self) -> u32 {
        self.seq.load(Ordering::Acquire)
    }

    #[inline]
    fn clock_mode(&self) -> i32 {
        self.clock_mode
    }

    #[inline]
    fn cycle_last(&self) -> u64 {
        self.cycle_last
    }

    #[inline]
    fn mask(&self) -> u64 {
        self.mask
    }

    #[inline]
    fn mult(&self) -> u32 {
        self.mult
    }

    #[inline]
    fn shift(&self) -> u32 {
        self.shift
    }

    #[inline]
    fn tz_minuteswest(&self) -> i32 {
        self.tz_minuteswest
    }

    #[inline]
    fn tz_dsttime(&self) -> i32 {
        self.tz_dsttime
    }

    #[inline]
    fn hrtimer_res(&self) -> u32 {
        self.hrtimer_res
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
pub union vdso_data_v5_9_union_1 {
    pub basetime: [vdso_timestamp; VDSO_BASES],
    pub offset: [timens_offset; VDSO_BASES],
}
