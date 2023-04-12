#![cfg_attr(feature = "sgx", no_std)]

#[cfg(feature = "sgx")]
extern crate sgx_types;
#[cfg(feature = "sgx")]
#[macro_use]
extern crate sgx_tstd as std;
#[cfg(feature = "sgx")]
extern crate sgx_libc as libc;
#[cfg(feature = "sgx")]
extern crate sgx_trts;

mod sys;

use errno::prelude::*;
use lazy_static::lazy_static;
use log::trace;
use std::convert::TryFrom;
use std::time::Duration;
use std::{hint, str};
use sys::*;

pub const NANOS_PER_SEC: u32 = 1_000_000_000;
pub const NANOS_PER_MILLI: u32 = 1_000_000;
pub const NANOS_PER_MICRO: u32 = 1_000;
pub const MILLIS_PER_SEC: u64 = 1_000;
pub const MICROS_PER_SEC: u64 = 1_000_000;

/// Clocks supported by the linux kernel, corresponding to clockid_t in Linux.
#[derive(Debug, Eq, PartialEq, Copy, Clone)]
#[allow(non_camel_case_types)]
pub enum ClockId {
    CLOCK_REALTIME = 0,
    CLOCK_MONOTONIC = 1,
    // vDSO doesn't support CLOCK_PROCESS_CPUTIME_ID.
    CLOCK_PROCESS_CPUTIME_ID = 2,
    // vDSO doesn't support CLOCK_THREAD_CPUTIME_ID.
    CLOCK_THREAD_CPUTIME_ID = 3,
    CLOCK_MONOTONIC_RAW = 4,
    CLOCK_REALTIME_COARSE = 5,
    CLOCK_MONOTONIC_COARSE = 6,
    CLOCK_BOOTTIME = 7,
}

impl TryFrom<i32> for ClockId {
    type Error = Error;

    fn try_from(clockid: i32) -> Result<Self> {
        Ok(match clockid {
            0 => ClockId::CLOCK_REALTIME,
            1 => ClockId::CLOCK_MONOTONIC,
            2 => ClockId::CLOCK_PROCESS_CPUTIME_ID,
            3 => ClockId::CLOCK_THREAD_CPUTIME_ID,
            4 => ClockId::CLOCK_MONOTONIC_RAW,
            5 => ClockId::CLOCK_REALTIME_COARSE,
            6 => ClockId::CLOCK_MONOTONIC_COARSE,
            7 => ClockId::CLOCK_BOOTTIME,
            _ => return_errno!(EINVAL, "Unsupported clockid"),
        })
    }
}

/// An abstraction of Linux vDSO provides the clock and time interface through Linux vDSO.
pub struct Vdso {
    vdso_data_ptr: VdsoDataPtr,
    // hres resolution for clock_getres
    hres_resolution: Option<Duration>,
    // coarse resolution for clock_getres
    coarse_resolution: Option<Duration>,
}

impl Vdso {
    /// Try to create a new Vdso by libc or SGX OCALL.
    ///
    /// # Examples
    ///
    /// ```
    /// use vdso_time::Vdso;
    /// let vdso = Vdso::new().unwrap();
    /// ```
    pub fn new() -> Result<Self> {
        let vdso_data_ptr = Self::get_vdso_data_ptr_from_host()?;
        let hres_resolution = clock_getres_slow(ClockId::CLOCK_MONOTONIC).ok();
        let coarse_resolution = clock_getres_slow(ClockId::CLOCK_MONOTONIC_COARSE).ok();
        let vdso = Self {
            vdso_data_ptr,
            hres_resolution,
            coarse_resolution,
        };
        vdso.check_accuracy()?;
        Ok(vdso)
    }

    #[cfg(feature = "sgx")]
    fn get_vdso_data_ptr_from_host() -> Result<VdsoDataPtr> {
        extern "C" {
            fn vdso_ocall_get_vdso_info(
                ret: *mut libc::c_int,
                vdso_addr: *mut libc::c_ulong,
                release: *mut libc::c_char,
                release_len: libc::c_int,
            ) -> sgx_types::sgx_status_t;
        }

        let mut vdso_addr: libc::c_ulong = 0;
        let mut release = [0 as libc::c_char; 65];
        let mut ret: libc::c_int = 0;
        unsafe {
            vdso_ocall_get_vdso_info(
                &mut ret as *mut _,
                &mut vdso_addr as *mut _,
                release.as_mut_ptr(),
                release.len() as _,
            );
        }
        if ret != 0 {
            return_errno!(EINVAL, "Vdso vdso_ocall_get_vdso_info() failed")
        }

        Self::match_kernel_version(vdso_addr, &release)
    }

    #[cfg(not(feature = "sgx"))]
    fn get_vdso_data_ptr_from_host() -> Result<VdsoDataPtr> {
        const AT_SYSINFO_EHDR: u64 = 33;
        let vdso_addr = unsafe { libc::getauxval(AT_SYSINFO_EHDR) };

        let mut utsname: libc::utsname = unsafe { std::mem::zeroed() };
        let ret = unsafe { libc::uname(&mut utsname as *mut _) };
        if ret != 0 {
            return_errno!(EINVAL, "Vdso get utsname failed");
        }
        let release = utsname.release;

        Self::match_kernel_version(vdso_addr, &release)
    }

    fn check_vdso_addr(vdso_addr: &u64) -> Result<()> {
        let vdso_addr = *vdso_addr;
        if vdso_addr == 0 {
            return_errno!(EFAULT, "Vdso vdso_addr is 0")
        }
        const VDSO_DATA_MAX_SIZE: u64 = 4 * PAGE_SIZE;
        if vdso_addr < VDSO_DATA_MAX_SIZE {
            return_errno!(EFAULT, "Vdso vdso_addr is less than vdso data size");
        }

        #[cfg(feature = "sgx")]
        if !sgx_trts::trts::rsgx_raw_is_outside_enclave(
            (vdso_addr - VDSO_DATA_MAX_SIZE) as *const u8,
            VDSO_DATA_MAX_SIZE as _,
        ) {
            return_errno!(EFAULT, "Vdso vdso_addr we got is not outside enclave")
        }

        Ok(())
    }

    fn match_kernel_version(vdso_addr: u64, release: &[libc::c_char]) -> Result<VdsoDataPtr> {
        Self::check_vdso_addr(&vdso_addr)?;

        // release, e.g., "5.9.6-050906-generic"
        let release = unsafe { &*(release as *const [i8] as *const [u8]) };
        let release = str::from_utf8(release);
        if release.is_err() {
            return_errno!(EINVAL, "Vdso get kernel release failed")
        }
        let mut release = release.unwrap().split(&['-', '.', ' '][..]);
        let version_big: u8 = release
            .next()
            .ok_or(errno!(EINVAL, "Vdso get kernel big version failed"))?
            .parse()?;
        let version_little: u8 = release
            .next()
            .ok_or(errno!(EINVAL, "Vdso get kernel little version failed"))?
            .parse()?;

        Ok(match (version_big, version_little) {
            (4, 0..=4) | (4, 7..=11) => VdsoDataPtr::V4_0(vdso_data_v4_0::vdsodata_ptr(vdso_addr)),
            (4, 5..=6) | (4, 12..=19) => VdsoDataPtr::V4_5(vdso_data_v4_5::vdsodata_ptr(vdso_addr)),
            (5, 0..=2) => VdsoDataPtr::V5_0(vdso_data_v5_0::vdsodata_ptr(vdso_addr)),
            (5, 3..=5) => VdsoDataPtr::V5_3(vdso_data_v5_3::vdsodata_ptr(vdso_addr)),
            (5, 6..=8) => VdsoDataPtr::V5_6(vdso_data_v5_6::vdsodata_ptr(vdso_addr)),
            (5, 9..=19) | (6, 0..=2) => VdsoDataPtr::V5_9(vdso_data_v5_9::vdsodata_ptr(vdso_addr)),
            (_, _) => return_errno!(EINVAL, "Vdso match kernel release failed"),
        })
    }

    /// Compare the results of Linux syscall and vdso to check whether vdso can support the clockid correctly.
    fn check_accuracy(&self) -> Result<()> {
        let vdso_supported_clockids = [
            ClockId::CLOCK_REALTIME,
            ClockId::CLOCK_MONOTONIC,
            ClockId::CLOCK_MONOTONIC_RAW,
            ClockId::CLOCK_REALTIME_COARSE,
            ClockId::CLOCK_MONOTONIC_COARSE,
            ClockId::CLOCK_BOOTTIME,
        ];
        const MAX_INACCURACY: Duration = Duration::from_millis(1);
        const MAX_RETRY_NUM: u32 = 3;
        for &clockid in vdso_supported_clockids.iter() {
            for retry_num in 0..MAX_RETRY_NUM {
                let time = match self.do_clock_gettime(clockid) {
                    Ok(time) => time,
                    Err(_) => break,
                };

                let host_time = match clock_gettime_slow(clockid) {
                    Ok(host_time) => host_time,
                    Err(_) => break,
                };

                let estimated_inaccuracy = match host_time.checked_sub(time) {
                    Some(diff) => diff,
                    None => return_errno!(EOPNOTSUPP, "Vdso can not provide valid time"),
                };
                if estimated_inaccuracy > MAX_INACCURACY {
                    if retry_num == MAX_RETRY_NUM - 1 {
                        return_errno!(EOPNOTSUPP, "Vdso reached max retry number");
                    }
                    continue;
                }
                trace!("Vdso support clock {:?}", clockid);
                break;
            }
        }
        trace!("Vdso passed check, init succeeded");
        Ok(())
    }

    /// Try to get time according to ClockId.
    /// Firstly try to get time through vDSO, if failed, then try fallback.
    ///
    /// # Examples
    ///
    /// ```
    /// use vdso_time::{Vdso, ClockId};
    /// let vdso = Vdso::new().unwrap();
    /// let time = vdso.clock_gettime(ClockId::CLOCK_MONOTONIC).unwrap();
    /// println!("{:?}", time);
    /// ```
    pub fn clock_gettime(&self, clockid: ClockId) -> Result<Duration> {
        self.do_clock_gettime(clockid)
            .or_else(|_| clock_gettime_slow(clockid))
    }

    /// Try to get time resolution according to ClockId.
    /// Firstly try to return resolution inside self, if failed, then try fallback.
    ///
    /// # Examples
    ///
    /// ```
    /// use vdso_time::{Vdso, ClockId};
    /// let vdso = Vdso::new().unwrap();
    /// let res = vdso.clock_getres(ClockId::CLOCK_MONOTONIC).unwrap();
    /// println!("{:?}", res);
    /// ```
    pub fn clock_getres(&self, clockid: ClockId) -> Result<Duration> {
        self.do_clock_getres(clockid)
            .or_else(|_| clock_getres_slow(clockid))
    }

    fn do_clock_gettime(&self, clockid: ClockId) -> Result<Duration> {
        match clockid {
            ClockId::CLOCK_REALTIME | ClockId::CLOCK_MONOTONIC | ClockId::CLOCK_BOOTTIME => {
                self.do_hres(ClockSource::CS_HRES_COARSE, clockid)
            }
            ClockId::CLOCK_MONOTONIC_RAW => self.do_hres(ClockSource::CS_RAW, clockid),
            ClockId::CLOCK_REALTIME_COARSE | ClockId::CLOCK_MONOTONIC_COARSE => {
                self.do_coarse(ClockSource::CS_HRES_COARSE, clockid)
            }
            // TODO: support CLOCK_PROCESS_CPUTIME_ID and CLOCK_THREAD_CPUTIME_ID.
            _ => return_errno!(EINVAL, "Unsupported clockid in do_clock_gettime()"),
        }
    }

    fn do_clock_getres(&self, clockid: ClockId) -> Result<Duration> {
        match clockid {
            ClockId::CLOCK_REALTIME
            | ClockId::CLOCK_MONOTONIC
            | ClockId::CLOCK_BOOTTIME
            | ClockId::CLOCK_MONOTONIC_RAW => self
                .hres_resolution
                .ok_or(errno!(EOPNOTSUPP, "hres_resolution is none")),
            ClockId::CLOCK_REALTIME_COARSE | ClockId::CLOCK_MONOTONIC_COARSE => self
                .coarse_resolution
                .ok_or(errno!(EOPNOTSUPP, "coarse_resolution is none")),
            // TODO: support CLOCK_PROCESS_CPUTIME_ID and CLOCK_THREAD_CPUTIME_ID.
            _ => return_errno!(EINVAL, "Unsupported clockid in do_clock_getres()"),
        }
    }

    fn vdso_data(&self, cs: ClockSource) -> &'static dyn VdsoData {
        match self.vdso_data_ptr {
            VdsoDataPtr::V4_0(ptr) => unsafe { &*(ptr) },
            VdsoDataPtr::V4_5(ptr) => unsafe { &*(ptr) },
            VdsoDataPtr::V5_0(ptr) => unsafe { &*(ptr) },
            VdsoDataPtr::V5_3(ptr) => unsafe { &*(ptr.add(cs as _)) },
            VdsoDataPtr::V5_6(ptr) => unsafe { &*(ptr.add(cs as _)) },
            VdsoDataPtr::V5_9(ptr) => unsafe { &*(ptr.add(cs as _)) },
        }
    }

    fn do_hres(&self, cs: ClockSource, clockid: ClockId) -> Result<Duration> {
        let vdso_data = self.vdso_data(cs);
        loop {
            let seq = vdso_data.seq();
            // if seq is odd, it might means that a concurrent update is in progress.
            // Hence, we do some instructions to spin waiting for seq to become even again.
            if seq & 1 != 0 {
                hint::spin_loop();
                continue;
            }

            // Make sure that all prior load-from-memory instructions have completed locally,
            // and no later instruction begins execution until LFENCE completes.
            // We want to make sure the execution order as followning:
            //     seq -> [cycles, cycle_last, mult, shift, sec, secs] -> seq
            // This LFENCE can ensure that the first seq is before [cycles, cycle_last, mult, shift, sec, secs]
            lfence();

            // Get hardware counter according to vdso_data's clock_mode.
            let cycles = Self::get_hw_counter(vdso_data)?;

            let cycle_last = vdso_data.cycle_last();
            let mult = vdso_data.mult();
            let shift = vdso_data.shift();
            let secs = vdso_data.sec(clockid as _)?;
            let mut nanos = vdso_data.nsec(clockid as _)?;

            if !Self::vdso_read_retry(vdso_data, seq) {
                // On x86 arch, the TSC can be slightly off across sockets,
                // which might cause cycles < cycle_last. Since they are u64 type,
                // cycles - cycle_last will panic in this case.
                // Hence we need to verify that cycles is greater than cycle_last.
                // If not then just use cycle_last, which is the base time of the
                // current conversion period.
                // And the vdso mask is always u64_MAX on x86, we don't need use mask.
                if cycles > cycle_last {
                    nanos += (cycles - cycle_last) * mult as u64
                }
                nanos = nanos >> shift;

                return Ok(Duration::new(secs, nanos as u32));
            }
        }
    }

    fn do_coarse(&self, cs: ClockSource, clockid: ClockId) -> Result<Duration> {
        let vdso_data = self.vdso_data(cs);
        loop {
            let seq = vdso_data.seq();
            // see comments in do_hres
            if seq & 1 != 0 {
                hint::spin_loop();
                continue;
            }

            // see comments in do_hres
            lfence();

            let secs = vdso_data.sec(clockid as _)?;
            let nanos = vdso_data.nsec(clockid as _)?;

            if !Self::vdso_read_retry(vdso_data, seq) {
                return Ok(Duration::new(secs, nanos as u32));
            }
        }
    }

    fn vdso_read_retry(vdso_data: &dyn VdsoData, old_seq: u32) -> bool {
        // Make sure that all prior load-from-memory instructions have completed locally,
        // and no later instruction begins execution until LFENCE completes
        lfence();

        old_seq != vdso_data.seq()
    }

    fn get_hw_counter(vdso_data: &dyn VdsoData) -> Result<u64> {
        let clock_mode = vdso_data.clock_mode();
        if clock_mode == VdsoClockMode::VDSO_CLOCKMODE_TSC as i32 {
            return Ok(rdtsc_ordered());
        } else if clock_mode == VdsoClockMode::VDSO_CLOCKMODE_PVCLOCK as i32 {
            // TODO: support pvclock
            return_errno!(
                EOPNOTSUPP,
                "VDSO_CLOCKMODE_PVCLOCK support is not implemented"
            );
        } else if clock_mode == VdsoClockMode::VDSO_CLOCKMODE_HVCLOCK as i32 {
            // TODO: support hvclock
            return_errno!(
                EOPNOTSUPP,
                "VDSO_CLOCKMODE_HVCLOCK support is not implemented"
            );
        } else if clock_mode == VdsoClockMode::VDSO_CLOCKMODE_TIMENS as i32 {
            // TODO: support timens
            return_errno!(
                EOPNOTSUPP,
                "VDSO_CLOCKMODE_TIMENS support is not implemented"
            );
        } else if clock_mode == VdsoClockMode::VDSO_CLOCKMODE_NONE as i32 {
            // In x86 Linux, the clock_mode will never be VDSO_CLOCKMODE_NONE.
            return_errno!(EINVAL, "The clock_mode must not be VDSO_CLOCKMODE_NONE");
        }
        return_errno!(EINVAL, "Unsupported clock_mode");
    }
}

unsafe impl Sync for Vdso {}
unsafe impl Send for Vdso {}

lazy_static! {
    static ref VDSO: Option<Vdso> = Vdso::new().map_or_else(
        |e| {
            trace!("{}", e);
            None
        },
        |v| Some(v)
    );
}

/// Try to get time according to ClockId.
/// Firstly try to get time through vDSO, if failed, then try fallback.
///
/// # Examples
///
/// ```
/// use vdso_time::ClockId;
///
/// let time = vdso_time::clock_gettime(ClockId::CLOCK_MONOTONIC).unwrap();
/// println!("{:?}", time);
/// ```
pub fn clock_gettime(clockid: ClockId) -> Result<Duration> {
    if VDSO.is_none() {
        clock_gettime_slow(clockid)
    } else {
        VDSO.as_ref().unwrap().clock_gettime(clockid)
    }
}

/// Try to get time resolution according to ClockId.
/// Firstly try to get time through vDSO, if failed, then try fallback.
///
/// # Examples
///
/// ```
/// use vdso_time::ClockId;
///
/// let time = vdso_time::clock_getres(ClockId::CLOCK_MONOTONIC).unwrap();
/// println!("{:?}", time);
/// ```
pub fn clock_getres(clockid: ClockId) -> Result<Duration> {
    if VDSO.is_none() {
        clock_getres_slow(clockid)
    } else {
        VDSO.as_ref().unwrap().clock_getres(clockid)
    }
}

fn clock_gettime_slow(clockid: ClockId) -> Result<Duration> {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };

    cfg_if::cfg_if! {
        if #[cfg(feature = "sgx")] {
            extern "C" {
                fn vdso_ocall_clock_gettime(
                    ret: *mut libc::c_int,
                    clockid: libc::c_int,
                    ts: *mut libc::timespec,
                ) -> sgx_types::sgx_status_t;
            }
            let mut ret: libc::c_int = 0;
            unsafe {
                vdso_ocall_clock_gettime(&mut ret as *mut _, clockid as _, &mut ts as *mut _);
            }
        } else {
            let ret = unsafe { libc::clock_gettime(clockid as _, &mut ts as *mut _) };
        }
    }

    if ret == 0 {
        Ok(Duration::new(ts.tv_sec as u64, ts.tv_nsec as u32))
    } else {
        return_errno!(EINVAL, "clock_gettime_slow failed")
    }
}

fn clock_getres_slow(clockid: ClockId) -> Result<Duration> {
    let mut res = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };

    cfg_if::cfg_if! {
        if #[cfg(feature = "sgx")] {
            extern "C" {
                fn vdso_ocall_clock_getres(
                    ret: *mut libc::c_int,
                    clockid: libc::c_int,
                    res: *mut libc::timespec,
                ) -> sgx_types::sgx_status_t;
            }
            let mut ret: libc::c_int = 0;
            unsafe {
                vdso_ocall_clock_getres(&mut ret as *mut _, clockid as _, &mut res as *mut _);
            }
        } else {
            let ret = unsafe { libc::clock_getres(clockid as _, &mut res as *mut _) };
        }
    }

    if ret == 0 {
        Ok(Duration::new(res.tv_sec as u64, res.tv_nsec as u32))
    } else {
        return_errno!(EINVAL, "clock_getres_slow failed")
    }
}

// All unit tests
#[cfg(test)]
#[allow(deprecated)]
mod tests {
    use super::*;
    use std::thread;

    const LOOPS: usize = 3;
    const SLEEP_DURATION: u64 = 10;
    const HRES_MAX_DIFF_NANOS: u64 = 50_000;
    const COARSE_MAX_DIFF_NANOS: u64 = 4_000_000;

    #[test]
    fn test_clock_gettime() {
        test_single_clock_gettime(ClockId::CLOCK_REALTIME_COARSE, COARSE_MAX_DIFF_NANOS);
        test_single_clock_gettime(ClockId::CLOCK_MONOTONIC_COARSE, COARSE_MAX_DIFF_NANOS);
        test_single_clock_gettime(ClockId::CLOCK_REALTIME, HRES_MAX_DIFF_NANOS);
        test_single_clock_gettime(ClockId::CLOCK_MONOTONIC, HRES_MAX_DIFF_NANOS);
        test_single_clock_gettime(ClockId::CLOCK_BOOTTIME, HRES_MAX_DIFF_NANOS);
        test_single_clock_gettime(ClockId::CLOCK_MONOTONIC_RAW, HRES_MAX_DIFF_NANOS);
    }

    fn test_single_clock_gettime(clockid: ClockId, max_diff_nanos: u64) {
        for _ in 0..LOOPS {
            let mut libc_tp = libc::timespec {
                tv_sec: 0,
                tv_nsec: 0,
            };
            unsafe { libc::clock_gettime(clockid as _, &mut libc_tp as *mut _) };
            let libc_time = Duration::new(libc_tp.tv_sec as u64, libc_tp.tv_nsec as u32);

            let vdso_time = clock_gettime(clockid).unwrap();

            assert!(vdso_time - libc_time <= Duration::from_nanos(max_diff_nanos));

            thread::sleep(Duration::from_millis(SLEEP_DURATION));
        }
    }

    #[test]
    fn test_clock_getres() {
        test_single_clock_getres(ClockId::CLOCK_REALTIME_COARSE);
        test_single_clock_getres(ClockId::CLOCK_MONOTONIC_COARSE);
        test_single_clock_getres(ClockId::CLOCK_REALTIME);
        test_single_clock_getres(ClockId::CLOCK_MONOTONIC);
        test_single_clock_getres(ClockId::CLOCK_BOOTTIME);
        test_single_clock_getres(ClockId::CLOCK_MONOTONIC_RAW);
    }

    fn test_single_clock_getres(clockid: ClockId) {
        for _ in 0..LOOPS {
            let mut libc_tp = libc::timespec {
                tv_sec: 0,
                tv_nsec: 0,
            };
            unsafe { libc::clock_getres(clockid as _, &mut libc_tp as *mut _) };

            let res = clock_getres(clockid).unwrap();

            assert_eq!(res.as_secs(), libc_tp.tv_sec as u64);
            assert_eq!(res.subsec_nanos(), libc_tp.tv_nsec as u32);
        }
    }

    #[test]
    fn test_monotonic() {
        let mut last_now = Duration::new(0, 0);
        for _ in 0..1_000_000 {
            let now = clock_gettime(ClockId::CLOCK_MONOTONIC).unwrap();
            assert!(now >= last_now);
            last_now = now;
        }
    }

    mod logger {
        use log::{Level, LevelFilter, Metadata, Record};

        #[ctor::ctor]
        fn auto_init() {
            log::set_logger(&LOGGER)
                .map(|()| log::set_max_level(LevelFilter::Trace))
                .expect("failed to init the logger");
        }

        static LOGGER: SimpleLogger = SimpleLogger;

        struct SimpleLogger;

        impl log::Log for SimpleLogger {
            fn enabled(&self, metadata: &Metadata) -> bool {
                metadata.level() <= Level::Trace
            }

            fn log(&self, record: &Record) {
                if self.enabled(record.metadata()) {
                    println!("[{}] {}", record.level(), record.args());
                }
            }

            fn flush(&self) {}
        }
    }
}
