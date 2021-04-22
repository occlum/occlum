use libc::{
    clockid_t, CLOCK_MONOTONIC, CLOCK_MONOTONIC_COARSE, CLOCK_REALTIME, CLOCK_REALTIME_COARSE,
};
use std::sync::atomic::{AtomicU32, Ordering};

pub const PAGE_SIZE: u64 = 4096;

pub const CLOCK_TAI: usize = 11;
pub const VDSO_BASES: usize = CLOCK_TAI + 1;

pub const NSEC_PER_USEC: u64 = 1000;
pub const NSEC_PER_SEC: u64 = 1000000000;

/// The timers is divided in 3 sets (HRES, COARSE, RAW) since Linux v5.3
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

pub trait VdsoData {
    fn sec(&self, clockid: clockid_t) -> Result<u64, ()>;
    fn nsec(&self, clockid: clockid_t) -> Result<u64, ()>;
    fn seq(&self) -> u32;
    fn clock_mode(&self) -> i32;
    fn cycle_last(&self) -> u64;
    fn mask(&self) -> u64;
    fn mult(&self) -> u32;
    fn shift(&self) -> u32;
    fn tz_minuteswest(&self) -> i32;
    fn tz_dsttime(&self) -> i32;

    fn vdsodata_ptr(vdso_addr: u64) -> *const Self
    where
        Self: Sized;
}

pub enum VdsoDataPtr {
    V4_0(*const vdso_data_v4_0),
    V4_5(*const vdso_data_v4_5),
    V5_0(*const vdso_data_v5_0),
    V5_3(*const vdso_data_v5_3),
    V5_6(*const vdso_data_v5_6),
    V5_9(*const vdso_data_v5_9),
}

// === Linux 4.0 - 4.4, 4.7 - 4.11 ===
// struct vsyscall_gtod_data

#[repr(C)]
pub struct vdso_data_v4_0 {
    pub seq: AtomicU32,

    pub vclock_mode: i32,
    pub cycle_last: u64,
    pub mask: u64,
    pub mult: u32,
    pub shift: u32,

    pub wall_time_snsec: u64,
    pub wall_time_sec: u64,
    pub monotonic_time_sec: u64,
    pub monotonic_time_snsec: u64,
    pub wall_time_coarse_sec: u64,
    pub wall_time_coarse_nsec: u64,
    pub monotonic_time_coarse_sec: u64,
    pub monotonic_time_coarse_nsec: u64,

    pub tz_minuteswest: i32,
    pub tz_dsttime: i32,
}

impl VdsoData for vdso_data_v4_0 {
    fn vdsodata_ptr(vdso_addr: u64) -> *const Self {
        (vdso_addr - 2 * PAGE_SIZE + 128) as *const Self
    }

    #[inline]
    fn sec(&self, clockid: clockid_t) -> Result<u64, ()> {
        match clockid {
            CLOCK_REALTIME => Ok(self.wall_time_sec),
            CLOCK_MONOTONIC => Ok(self.monotonic_time_sec),
            CLOCK_REALTIME_COARSE => Ok(self.wall_time_coarse_sec),
            CLOCK_MONOTONIC_COARSE => Ok(self.monotonic_time_coarse_sec),
            _ => Err(()),
        }
    }

    #[inline]
    fn nsec(&self, clockid: clockid_t) -> Result<u64, ()> {
        match clockid {
            CLOCK_REALTIME => Ok(self.wall_time_snsec),
            CLOCK_MONOTONIC => Ok(self.monotonic_time_snsec),
            CLOCK_REALTIME_COARSE => Ok(self.wall_time_coarse_nsec),
            CLOCK_MONOTONIC_COARSE => Ok(self.monotonic_time_coarse_nsec),
            _ => Err(()),
        }
    }

    #[inline]
    fn seq(&self) -> u32 {
        self.seq.load(Ordering::Acquire)
    }

    #[inline]
    fn clock_mode(&self) -> i32 {
        self.vclock_mode
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
}

// === Linux 4.5 - 4.6, 4.12 - 4.19 ===
// struct vsyscall_gtod_data

#[repr(C)]
pub struct vdso_data_v4_5 {
    pub seq: AtomicU32,

    pub vclock_mode: i32,
    pub cycle_last: u64,
    pub mask: u64,
    pub mult: u32,
    pub shift: u32,

    pub wall_time_snsec: u64,
    pub wall_time_sec: u64,
    pub monotonic_time_sec: u64,
    pub monotonic_time_snsec: u64,
    pub wall_time_coarse_sec: u64,
    pub wall_time_coarse_nsec: u64,
    pub monotonic_time_coarse_sec: u64,
    pub monotonic_time_coarse_nsec: u64,

    pub tz_minuteswest: i32,
    pub tz_dsttime: i32,
}

impl VdsoData for vdso_data_v4_5 {
    fn vdsodata_ptr(vdso_addr: u64) -> *const Self {
        (vdso_addr - 3 * PAGE_SIZE + 128) as *const Self
    }

    #[inline]
    fn sec(&self, clockid: clockid_t) -> Result<u64, ()> {
        match clockid {
            CLOCK_REALTIME => Ok(self.wall_time_sec),
            CLOCK_MONOTONIC => Ok(self.monotonic_time_sec),
            CLOCK_REALTIME_COARSE => Ok(self.wall_time_coarse_sec),
            CLOCK_MONOTONIC_COARSE => Ok(self.monotonic_time_coarse_sec),
            _ => Err(()),
        }
    }

    #[inline]
    fn nsec(&self, clockid: clockid_t) -> Result<u64, ()> {
        match clockid {
            CLOCK_REALTIME => Ok(self.wall_time_snsec),
            CLOCK_MONOTONIC => Ok(self.monotonic_time_snsec),
            CLOCK_REALTIME_COARSE => Ok(self.wall_time_coarse_nsec),
            CLOCK_MONOTONIC_COARSE => Ok(self.monotonic_time_coarse_nsec),
            _ => Err(()),
        }
    }

    #[inline]
    fn seq(&self) -> u32 {
        self.seq.load(Ordering::Acquire)
    }

    #[inline]
    fn clock_mode(&self) -> i32 {
        self.vclock_mode
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
}

// === Linux 5.0 - 5.2 ===
// struct vsyscall_gtod_data

#[repr(C)]
pub struct vdso_data_v5_0 {
    pub seq: AtomicU32,

    pub vclock_mode: i32,
    pub cycle_last: u64,
    pub mask: u64,
    pub mult: u32,
    pub shift: u32,

    pub basetime: [vgtod_ts; VDSO_BASES],

    pub tz_minuteswest: i32,
    pub tz_dsttime: i32,
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct vgtod_ts {
    pub sec: u64,
    pub nsec: u64,
}

impl VdsoData for vdso_data_v5_0 {
    fn vdsodata_ptr(vdso_addr: u64) -> *const Self {
        (vdso_addr - 3 * PAGE_SIZE + 128) as *const Self
    }

    #[inline]
    fn sec(&self, clockid: clockid_t) -> Result<u64, ()> {
        Ok(self.basetime[clockid as usize].sec)
    }

    #[inline]
    fn nsec(&self, clockid: clockid_t) -> Result<u64, ()> {
        Ok(self.basetime[clockid as usize].nsec)
    }

    #[inline]
    fn seq(&self) -> u32 {
        self.seq.load(Ordering::Acquire)
    }

    #[inline]
    fn clock_mode(&self) -> i32 {
        self.vclock_mode
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
}

// === Linux 5.3 - 5.5 ===
// struct vdso_data

#[repr(C)]
pub struct vdso_data_v5_3 {
    pub seq: AtomicU32,

    pub clock_mode: i32,
    pub cycle_last: u64,
    pub mask: u64,
    pub mult: u32,
    pub shift: u32,

    pub basetime: [vdso_timestamp; VDSO_BASES],

    pub tz_minuteswest: i32,
    pub tz_dsttime: i32,
    pub hrtimer_res: u32,
    pub __unused: u32,
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct vdso_timestamp {
    pub sec: u64,
    pub nsec: u64,
}

impl VdsoData for vdso_data_v5_3 {
    fn vdsodata_ptr(vdso_addr: u64) -> *const Self {
        (vdso_addr - 3 * PAGE_SIZE + 128) as *const Self
    }

    #[inline]
    fn sec(&self, clockid: clockid_t) -> Result<u64, ()> {
        Ok(self.basetime[clockid as usize].sec)
    }

    #[inline]
    fn nsec(&self, clockid: clockid_t) -> Result<u64, ()> {
        Ok(self.basetime[clockid as usize].nsec)
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
}

// === Linux 5.6 - 5.8 ===
// struct vdso_data

#[repr(C)]
pub struct vdso_data_v5_6 {
    pub seq: AtomicU32,

    pub clock_mode: i32,
    pub cycle_last: u64,
    pub mask: u64,
    pub mult: u32,
    pub shift: u32,

    pub union_1: vdso_data_v5_6_union_1,

    pub tz_minuteswest: i32,
    pub tz_dsttime: i32,
    pub hrtimer_res: u32,
    pub __unused: u32,
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct timens_offset {
    pub sec: i64,
    pub nsec: u64,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub union vdso_data_v5_6_union_1 {
    pub basetime: [vdso_timestamp; VDSO_BASES],
    pub offset: [timens_offset; VDSO_BASES],
}

impl VdsoData for vdso_data_v5_6 {
    fn vdsodata_ptr(vdso_addr: u64) -> *const Self {
        (vdso_addr - 4 * PAGE_SIZE + 128) as *const Self
    }

    #[inline]
    fn sec(&self, clockid: clockid_t) -> Result<u64, ()> {
        unsafe { Ok(self.union_1.basetime[clockid as usize].sec) }
    }

    #[inline]
    fn nsec(&self, clockid: clockid_t) -> Result<u64, ()> {
        unsafe { Ok(self.union_1.basetime[clockid as usize].nsec) }
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
}

// === Linux 5.9 - 5.12 ===
// struct vdso_data

#[repr(C)]
pub struct vdso_data_v5_9 {
    pub seq: AtomicU32,

    pub clock_mode: i32,
    pub cycle_last: u64,
    pub mask: u64,
    pub mult: u32,
    pub shift: u32,

    pub union_1: vdso_data_v5_6_union_1,

    pub tz_minuteswest: i32,
    pub tz_dsttime: i32,
    pub hrtimer_res: u32,
    pub __unused: u32,

    pub arch_data: arch_vdso_data,
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct arch_vdso_data {}

impl VdsoData for vdso_data_v5_9 {
    fn vdsodata_ptr(vdso_addr: u64) -> *const Self {
        (vdso_addr - 4 * PAGE_SIZE + 128) as *const Self
    }

    #[inline]
    fn sec(&self, clockid: clockid_t) -> Result<u64, ()> {
        unsafe { Ok(self.union_1.basetime[clockid as usize].sec) }
    }

    #[inline]
    fn nsec(&self, clockid: clockid_t) -> Result<u64, ()> {
        unsafe { Ok(self.union_1.basetime[clockid as usize].nsec) }
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
}
