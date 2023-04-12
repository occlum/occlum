use super::*;
use std::sync::atomic::{AtomicU32, Ordering};

pub const PAGE_SIZE: u64 = 4096;

pub const CLOCK_TAI: usize = 11;
pub const VDSO_BASES: usize = CLOCK_TAI + 1;

#[cfg(not(any(arget_arch = "x86", target_arch = "x86_64")))]
compile_error!("Only support x86 or x86_64 architecture now.");

/// Reads the current value of the processor’s time-stamp counter.
///
/// The processor monotonically increments the time-stamp counter MSR every clock cycle
/// and resets it to 0 whenever the processor is reset.
///
/// The RDTSC instruction is not a serializing instruction. It does not necessarily
/// wait until all previous instructions have been executed before reading the counter.
/// Similarly, subsequent instructions may begin execution before the read operation is performed.
pub fn rdtsc() -> u64 {
    cfg_if::cfg_if! {
        if #[cfg(target_arch = "x86_64")] {
            unsafe { core::arch::x86_64::_rdtsc() as u64 }
        } else if #[cfg(target_arch = "x86")] {
            unsafe { core::arch::x86::_rdtsc() as u64 }
        }
    }
}

/// Reads the current value of the processor’s time-stamp counter.
///
/// The processor monotonically increments the time-stamp counter MSR every clock cycle
/// and resets it to 0 whenever the processor is reset.
/// The RDTSCP instruction waits until all previous instructions have been executed before
/// reading the counter. However, subsequent instructions may begin execution before
/// the read operation is performed.
#[allow(dead_code)]
pub fn rdtscp() -> u64 {
    cfg_if::cfg_if! {
        if #[cfg(target_arch = "x86_64")] {
            let mut aux: u32 = 0;
            unsafe { core::arch::x86_64::__rdtscp(&mut aux) as u64 }
        } else if #[cfg(target_arch = "x86")] {
            let mut aux: u32 = 0;
            unsafe { core::arch::x86::__rdtscp(&mut aux) as u64 }
        }
    }
}

/// Performs a serializing operation on all load-from-memory instructions
/// that were issued prior to this instruction.
///
/// Guarantees that every load instruction that precedes, in program order,
/// is globally visible before any load instruction which follows the fence in program order.
pub fn lfence() {
    cfg_if::cfg_if! {
        if #[cfg(target_arch = "x86_64")] {
            unsafe { core::arch::x86_64::_mm_lfence() }
        } else if #[cfg(target_arch = "x86")] {
            unsafe { core::arch::x86::_mm_lfence() }
        }
    }
}

/// Read the current TSC in program order.
///
/// The RDTSC instruction might not be ordered relative to memory access.
/// But an RDTSC immediately after an appropriate barrier appears to be ordered as a normal load.
/// Hence, we could use a barrier before RDTSC to get ordered TSC.
///
/// We also can just use RDTSCP, which is also ordered.
pub fn rdtsc_ordered() -> u64 {
    lfence();
    rdtsc()
}

/// The timers is divided in 3 sets (HRES, COARSE, RAW) since Linux v5.3.
/// CS_HRES_COARSE refers to the first two and CS_RAW to the third.
#[derive(Debug, Copy, Clone)]
#[allow(non_camel_case_types)]
pub enum ClockSource {
    CS_HRES_COARSE = 0,
    CS_RAW = 1,
}

#[derive(Debug, Copy, Clone)]
#[allow(non_camel_case_types)]
pub enum VdsoClockMode {
    VDSO_CLOCKMODE_NONE = 0,
    VDSO_CLOCKMODE_TSC = 1,
    VDSO_CLOCKMODE_PVCLOCK = 2,
    VDSO_CLOCKMODE_HVCLOCK = 3,
    VDSO_CLOCKMODE_TIMENS = i32::MAX as isize,
}

// Struct VdsoDataPtr must impl this trait to unify vdso_data interface of different linux verisons.
pub trait VdsoData {
    fn sec(&self, clockid: ClockId) -> Result<u64>;
    fn nsec(&self, clockid: ClockId) -> Result<u64>;
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
    // === Linux 4.0 - 4.4, 4.7 - 4.11 ===
    V4_0(*const vdso_data_v4_0),
    // === Linux 4.5 - 4.6, 4.12 - 4.19 ===
    V4_5(*const vdso_data_v4_5),
    // === Linux 5.0 - 5.2 ===
    V5_0(*const vdso_data_v5_0),
    // === Linux 5.3 - 5.5 ===
    V5_3(*const vdso_data_v5_3),
    // === Linux 5.6 - 5.8 ===
    V5_6(*const vdso_data_v5_6),
    // === Linux 5.9 - 6.2 ===
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

    fn sec(&self, clockid: ClockId) -> Result<u64> {
        match clockid {
            ClockId::CLOCK_REALTIME => Ok(self.wall_time_sec),
            ClockId::CLOCK_MONOTONIC => Ok(self.monotonic_time_sec),
            ClockId::CLOCK_REALTIME_COARSE => Ok(self.wall_time_coarse_sec),
            ClockId::CLOCK_MONOTONIC_COARSE => Ok(self.monotonic_time_coarse_sec),
            _ => return_errno!(EINVAL, "Unsupported clockid in sec()"),
        }
    }

    fn nsec(&self, clockid: ClockId) -> Result<u64> {
        match clockid {
            ClockId::CLOCK_REALTIME => Ok(self.wall_time_snsec),
            ClockId::CLOCK_MONOTONIC => Ok(self.monotonic_time_snsec),
            ClockId::CLOCK_REALTIME_COARSE => Ok(self.wall_time_coarse_nsec),
            ClockId::CLOCK_MONOTONIC_COARSE => Ok(self.monotonic_time_coarse_nsec),
            _ => return_errno!(EINVAL, "Unsupported clockid in nsec()"),
        }
    }

    fn seq(&self) -> u32 {
        self.seq.load(Ordering::Relaxed)
    }

    fn clock_mode(&self) -> i32 {
        self.vclock_mode
    }

    fn cycle_last(&self) -> u64 {
        self.cycle_last
    }

    fn mask(&self) -> u64 {
        self.mask
    }

    fn mult(&self) -> u32 {
        self.mult
    }

    fn shift(&self) -> u32 {
        self.shift
    }

    fn tz_minuteswest(&self) -> i32 {
        self.tz_minuteswest
    }

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

    fn sec(&self, clockid: ClockId) -> Result<u64> {
        match clockid {
            ClockId::CLOCK_REALTIME => Ok(self.wall_time_sec),
            ClockId::CLOCK_MONOTONIC => Ok(self.monotonic_time_sec),
            ClockId::CLOCK_REALTIME_COARSE => Ok(self.wall_time_coarse_sec),
            ClockId::CLOCK_MONOTONIC_COARSE => Ok(self.monotonic_time_coarse_sec),
            _ => return_errno!(EINVAL, "Unsupported clockid in sec()"),
        }
    }

    fn nsec(&self, clockid: ClockId) -> Result<u64> {
        match clockid {
            ClockId::CLOCK_REALTIME => Ok(self.wall_time_snsec),
            ClockId::CLOCK_MONOTONIC => Ok(self.monotonic_time_snsec),
            ClockId::CLOCK_REALTIME_COARSE => Ok(self.wall_time_coarse_nsec),
            ClockId::CLOCK_MONOTONIC_COARSE => Ok(self.monotonic_time_coarse_nsec),
            _ => return_errno!(EINVAL, "Unsupported clockid in nsec()"),
        }
    }

    fn seq(&self) -> u32 {
        self.seq.load(Ordering::Relaxed)
    }

    fn clock_mode(&self) -> i32 {
        self.vclock_mode
    }

    fn cycle_last(&self) -> u64 {
        self.cycle_last
    }

    fn mask(&self) -> u64 {
        self.mask
    }

    fn mult(&self) -> u32 {
        self.mult
    }

    fn shift(&self) -> u32 {
        self.shift
    }

    fn tz_minuteswest(&self) -> i32 {
        self.tz_minuteswest
    }

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

    fn sec(&self, clockid: ClockId) -> Result<u64> {
        Ok(self.basetime[clockid as usize].sec)
    }

    fn nsec(&self, clockid: ClockId) -> Result<u64> {
        Ok(self.basetime[clockid as usize].nsec)
    }

    fn seq(&self) -> u32 {
        self.seq.load(Ordering::Relaxed)
    }

    fn clock_mode(&self) -> i32 {
        self.vclock_mode
    }

    fn cycle_last(&self) -> u64 {
        self.cycle_last
    }

    fn mask(&self) -> u64 {
        self.mask
    }

    fn mult(&self) -> u32 {
        self.mult
    }

    fn shift(&self) -> u32 {
        self.shift
    }

    fn tz_minuteswest(&self) -> i32 {
        self.tz_minuteswest
    }

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

    fn sec(&self, clockid: ClockId) -> Result<u64> {
        Ok(self.basetime[clockid as usize].sec)
    }

    fn nsec(&self, clockid: ClockId) -> Result<u64> {
        Ok(self.basetime[clockid as usize].nsec)
    }

    fn seq(&self) -> u32 {
        self.seq.load(Ordering::Relaxed)
    }

    fn clock_mode(&self) -> i32 {
        self.clock_mode
    }

    fn cycle_last(&self) -> u64 {
        self.cycle_last
    }

    fn mask(&self) -> u64 {
        self.mask
    }

    fn mult(&self) -> u32 {
        self.mult
    }

    fn shift(&self) -> u32 {
        self.shift
    }

    fn tz_minuteswest(&self) -> i32 {
        self.tz_minuteswest
    }

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

    fn sec(&self, clockid: ClockId) -> Result<u64> {
        unsafe { Ok(self.union_1.basetime[clockid as usize].sec) }
    }

    fn nsec(&self, clockid: ClockId) -> Result<u64> {
        unsafe { Ok(self.union_1.basetime[clockid as usize].nsec) }
    }

    fn seq(&self) -> u32 {
        self.seq.load(Ordering::Relaxed)
    }

    fn clock_mode(&self) -> i32 {
        self.clock_mode
    }

    fn cycle_last(&self) -> u64 {
        self.cycle_last
    }

    fn mask(&self) -> u64 {
        self.mask
    }

    fn mult(&self) -> u32 {
        self.mult
    }

    fn shift(&self) -> u32 {
        self.shift
    }

    fn tz_minuteswest(&self) -> i32 {
        self.tz_minuteswest
    }

    fn tz_dsttime(&self) -> i32 {
        self.tz_dsttime
    }
}

// === Linux 5.9 - 5.19, 6.0 - 6.2 ===
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

    fn sec(&self, clockid: ClockId) -> Result<u64> {
        unsafe { Ok(self.union_1.basetime[clockid as usize].sec) }
    }

    fn nsec(&self, clockid: ClockId) -> Result<u64> {
        unsafe { Ok(self.union_1.basetime[clockid as usize].nsec) }
    }

    fn seq(&self) -> u32 {
        self.seq.load(Ordering::Relaxed)
    }

    fn clock_mode(&self) -> i32 {
        self.clock_mode
    }

    fn cycle_last(&self) -> u64 {
        self.cycle_last
    }

    fn mask(&self) -> u64 {
        self.mask
    }

    fn mult(&self) -> u32 {
        self.mult
    }

    fn shift(&self) -> u32 {
        self.shift
    }

    fn tz_minuteswest(&self) -> i32 {
        self.tz_minuteswest
    }

    fn tz_dsttime(&self) -> i32 {
        self.tz_dsttime
    }
}
