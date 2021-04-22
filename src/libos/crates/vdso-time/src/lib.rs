#![cfg_attr(feature = "sgx", no_std)]
#![feature(asm)]
#![feature(llvm_asm)]

#[cfg(feature = "sgx")]
extern crate sgx_types;
#[cfg(feature = "sgx")]
#[macro_use]
extern crate sgx_tstd as std;
#[cfg(feature = "sgx")]
extern crate sgx_libc as libc;

mod sys;

pub use libc::{
    clockid_t, time_t, timespec, timeval, CLOCK_BOOTTIME, CLOCK_MONOTONIC, CLOCK_MONOTONIC_COARSE,
    CLOCK_MONOTONIC_RAW, CLOCK_REALTIME, CLOCK_REALTIME_COARSE,
};
use std::str;
use std::sync::atomic::{self, Ordering};
pub use sys::timezone;
use sys::*;

pub struct Vdso {
    vdso_data_ptr: VdsoDataPtr,
    // hres resolution for clock_getres
    hres_resolution: Option<i64>,
    // coarse resolution for clock_getres
    coarse_resolution: Option<i64>,
    // If support_clocks[clockid] is true, indicate that the corresponding clockid is supported
    support_clocks: [bool; VDSO_BASES],
}

impl Vdso {
    pub fn new() -> Result<Self, ()> {
        let mut support_clocks = [false; VDSO_BASES];
        let clockids = [
            CLOCK_REALTIME,
            CLOCK_MONOTONIC,
            CLOCK_MONOTONIC_RAW,
            CLOCK_REALTIME_COARSE,
            CLOCK_MONOTONIC_COARSE,
            CLOCK_BOOTTIME,
        ];

        #[cfg(not(feature = "sgx"))]
        let (vdso_addr, hres_resolution, coarse_resolution, tss, release) = {
            const AT_SYSINFO_EHDR: u64 = 33;
            let vdso_addr = unsafe { libc::getauxval(AT_SYSINFO_EHDR) };

            let mut tp = timespec {
                tv_sec: 0,
                tv_nsec: 0,
            };
            let ret = unsafe { libc::clock_getres(CLOCK_REALTIME, &mut tp as *mut _) };
            let hres_resolution = if ret == 0 { Some(tp.tv_nsec) } else { None };
            let ret = unsafe { libc::clock_getres(CLOCK_REALTIME_COARSE, &mut tp as *mut _) };
            let coarse_resolution = if ret == 0 { Some(tp.tv_nsec) } else { None };

            let mut utsname: libc::utsname = unsafe { std::mem::zeroed() };
            let ret = unsafe { libc::uname(&mut utsname as *mut _) };
            if ret != 0 {
                return Err(());
            }

            let mut tss = [timespec {
                tv_sec: 0,
                tv_nsec: 0,
            }; VDSO_BASES as usize];
            for &clockid in &clockids {
                unsafe {
                    if libc::clock_gettime(clockid, &mut tss[clockid as usize] as *mut _) != 0 {
                        tss[clockid as usize].tv_sec = 0;
                        tss[clockid as usize].tv_nsec = 0;
                    }
                }
            }

            (
                vdso_addr,
                hres_resolution,
                coarse_resolution,
                tss,
                utsname.release,
            )
        };

        #[cfg(feature = "sgx")]
        let (vdso_addr, hres_resolution, coarse_resolution, tss, release) = {
            extern "C" {
                fn vdso_ocall_get_vdso_info(
                    ret: *mut libc::c_int,
                    vdso_addr: *mut libc::c_ulong,
                    hres_resolution: *mut libc::c_long,
                    coarse_resolution: *mut libc::c_long,
                    release: *mut libc::c_char,
                    release_len: libc::c_int,
                    tss: *mut timespec,
                    tss_len: libc::c_int,
                ) -> sgx_types::sgx_status_t;
            }

            let mut vdso_addr: libc::c_ulong = 0;
            let mut hres_resolution: libc::c_long = 0;
            let mut coarse_resolution: libc::c_long = 0;
            let mut release = [0 as libc::c_char; 65];
            let mut tss = [timespec {
                tv_sec: 0,
                tv_nsec: 0,
            }; VDSO_BASES as usize];
            let mut ret: libc::c_int = 0;
            unsafe {
                vdso_ocall_get_vdso_info(
                    &mut ret as *mut _,
                    &mut vdso_addr as *mut _,
                    &mut hres_resolution as *mut _,
                    &mut coarse_resolution as *mut _,
                    release.as_mut_ptr(),
                    release.len() as _,
                    tss.as_mut_ptr(),
                    tss.len() as _,
                );
            }
            if ret != 0 {
                return Err(());
            }

            let hres_resolution = if hres_resolution != 0 {
                Some(hres_resolution)
            } else {
                None
            };
            let coarse_resolution = if coarse_resolution != 0 {
                Some(coarse_resolution)
            } else {
                None
            };

            (vdso_addr, hres_resolution, coarse_resolution, tss, release)
        };

        if vdso_addr == 0 {
            return Err(());
        }

        // release, e.g., "5.9.6-050906-generic"
        let release = unsafe { &*(&release as *const [i8] as *const [u8]) };
        let release = str::from_utf8(release);
        if release.is_err() {
            return Err(());
        }
        let mut release = release.unwrap().split(&['-', '.', ' '][..]);
        let version_big: u8 = release.next().unwrap_or("0").parse().unwrap_or(0);
        let version_little: u8 = release.next().unwrap_or("0").parse().unwrap_or(0);

        let vdso_data_ptr = match (version_big, version_little) {
            (4, 0..=4) | (4, 7..=11) => VdsoDataPtr::V4_0(vdso_data_v4_0::vdsodata_ptr(vdso_addr)),
            (4, 5..=6) | (4, 12..=19) => VdsoDataPtr::V4_5(vdso_data_v4_5::vdsodata_ptr(vdso_addr)),
            (5, 0..=2) => VdsoDataPtr::V5_0(vdso_data_v5_0::vdsodata_ptr(vdso_addr)),
            (5, 3..=5) => VdsoDataPtr::V5_3(vdso_data_v5_3::vdsodata_ptr(vdso_addr)),
            (5, 6..=8) => VdsoDataPtr::V5_6(vdso_data_v5_6::vdsodata_ptr(vdso_addr)),
            (5, _) => VdsoDataPtr::V5_9(vdso_data_v5_9::vdsodata_ptr(vdso_addr)),
            (_, _) => return Err(()),
        };

        // If linux support a clockid, then we think vdso support it too temporaryly.
        // We will verify whether vdso can support this clockid correctly later.
        for &clockid in &clockids {
            if !(tss[clockid as usize].tv_sec == 0 && tss[clockid as usize].tv_nsec == 0) {
                support_clocks[clockid as usize] = true;
            }
        }

        let mut vdso = Self {
            vdso_data_ptr,
            hres_resolution,
            coarse_resolution,
            support_clocks,
        };

        // Compare the results of Linux and vdso
        // to check whether vdso can support the clockid correctly.
        for &clockid in &clockids {
            if vdso.support_clocks[clockid as usize] {
                let mut tp = timespec {
                    tv_sec: 0,
                    tv_nsec: 0,
                };
                if vdso.clock_gettime(clockid, &mut tp as *mut _).is_err() {
                    vdso.support_clocks[clockid as usize] = false;
                }
                let diff = (tp.tv_sec - tss[clockid as usize].tv_sec) * NSEC_PER_SEC as i64
                    + (tp.tv_nsec - tss[clockid as usize].tv_nsec);
                if diff < 0 || diff > 10000 * NSEC_PER_USEC as i64 {
                    vdso.support_clocks[clockid as usize] = false;
                }
            }
        }

        Ok(vdso)
    }

    // Linux time(): time_t time(time_t *tloc);
    pub fn time(&self, tloc: *mut time_t) -> Result<time_t, ()> {
        let clockid = CLOCK_REALTIME;
        if !self.support_clocks[clockid as usize] {
            return Err(());
        }

        let vdso_data = self.vdso_data(ClockSource::CS_HRES_COARSE);
        let t: time_t = vdso_data.sec(clockid)? as _;
        if !tloc.is_null() {
            unsafe {
                *tloc = t;
            }
        }
        Ok(t)
    }

    // Linux gettimeofday(): int gettimeofday(struct timeval *tv, struct timezone *tz);
    pub fn gettimeofday(&self, tv: *mut timeval, tz: *mut timezone) -> Result<i32, ()> {
        let clockid = CLOCK_REALTIME;
        if !self.support_clocks[clockid as usize] {
            return Err(());
        }

        if !tv.is_null() {
            let mut tp = timespec {
                tv_sec: 0,
                tv_nsec: 0,
            };
            self.do_hres(ClockSource::CS_HRES_COARSE, clockid, &mut tp)?;
            unsafe {
                (*tv).tv_sec = tp.tv_sec;
                (*tv).tv_usec = tp.tv_nsec / NSEC_PER_USEC as i64;
            }
        }

        if !tz.is_null() {
            let vdso_data = self.vdso_data(ClockSource::CS_HRES_COARSE);
            unsafe {
                (*tz).tz_minuteswest = vdso_data.tz_minuteswest();
                (*tz).tz_dsttime = vdso_data.tz_dsttime();
            }
        }

        Ok(0)
    }

    // Linux clock_gettime(): int clock_gettime(clockid_t clockid, struct timespec *tp);
    pub fn clock_gettime(&self, clockid: clockid_t, tp: *mut timespec) -> Result<i32, ()> {
        if !self.support_clocks[clockid as usize] {
            return Err(());
        }

        match clockid {
            CLOCK_REALTIME | CLOCK_MONOTONIC | CLOCK_BOOTTIME => {
                self.do_hres(ClockSource::CS_HRES_COARSE, clockid, tp)
            }
            CLOCK_MONOTONIC_RAW => self.do_hres(ClockSource::CS_RAW, clockid, tp),
            CLOCK_REALTIME_COARSE | CLOCK_MONOTONIC_COARSE => {
                self.do_coarse(ClockSource::CS_HRES_COARSE, clockid, tp)
            }
            _ => Err(()),
        }
    }

    // Linux clock_getres(): int clock_getres(clockid_t clockid, struct timespec *res);
    pub fn clock_getres(&self, clockid: clockid_t, res: *mut timespec) -> Result<i32, ()> {
        let ns = match clockid {
            CLOCK_REALTIME | CLOCK_MONOTONIC | CLOCK_BOOTTIME | CLOCK_MONOTONIC_RAW => {
                if self.hres_resolution.is_none() {
                    return Err(());
                }
                self.hres_resolution.unwrap()
            }
            CLOCK_REALTIME_COARSE | CLOCK_MONOTONIC_COARSE => {
                if self.coarse_resolution.is_none() {
                    return Err(());
                }
                self.coarse_resolution.unwrap()
            }
            _ => return Err(()),
        };

        unsafe {
            (*res).tv_sec = 0;
            (*res).tv_nsec = ns;
        }

        Ok(0)
    }

    #[inline]
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

    fn do_hres(&self, cs: ClockSource, clockid: clockid_t, tp: *mut timespec) -> Result<i32, ()> {
        let vdso_data = self.vdso_data(cs);
        loop {
            let seq = vdso_data.seq();

            atomic::fence(Ordering::Acquire);

            if vdso_data.clock_mode() == vdso_clock_mode::VDSO_CLOCKMODE_NONE as i32 {
                return Err(());
            }

            let cycles = {
                let upper: u64;
                let lower: u64;
                unsafe {
                    llvm_asm!("rdtscp"
                         : "={rax}"(lower),
                           "={rdx}"(upper)
                         :
                         :
                         : "volatile"
                    );
                }
                upper << 32 | lower
            };

            let sec = vdso_data.sec(clockid)?;
            let mut ns = vdso_data.nsec(clockid)?;
            ns += ((cycles - vdso_data.cycle_last()) & vdso_data.mask()) * vdso_data.mult() as u64;
            ns = ns >> vdso_data.shift();

            if !Self::vdso_read_retry(vdso_data, seq) {
                unsafe {
                    (*tp).tv_sec = (sec + ns / NSEC_PER_SEC) as i64;
                    (*tp).tv_nsec = (ns % NSEC_PER_SEC) as i64;
                }
                break;
            }
        }
        Ok(0)
    }

    fn do_coarse(&self, cs: ClockSource, clockid: clockid_t, tp: *mut timespec) -> Result<i32, ()> {
        let vdso_data = self.vdso_data(cs);
        loop {
            let seq = vdso_data.seq();

            atomic::fence(Ordering::Acquire);

            unsafe {
                (*tp).tv_sec = vdso_data.sec(clockid)? as i64;
                (*tp).tv_nsec = vdso_data.nsec(clockid)? as i64;
            }

            if !Self::vdso_read_retry(vdso_data, seq) {
                break;
            }
        }
        Ok(0)
    }

    #[inline]
    fn vdso_read_retry(vdso_data: &dyn VdsoData, old_seq: u32) -> bool {
        atomic::fence(Ordering::Acquire);
        old_seq != vdso_data.seq()
    }
}

unsafe impl Sync for Vdso {}
unsafe impl Send for Vdso {}

// All unit tests
#[cfg(test)]
mod tests {
    use super::*;
    use std::{thread, time};

    const LOOPS: usize = 3;
    const SLEEP_DURATION: u64 = 10;
    const MAX_DIFF_NSEC: u64 = 10000;
    const USEC_PER_SEC: u64 = 1000000;

    #[test]
    fn test_time() {
        let vdso = Vdso::new().unwrap();
        for _ in 0..LOOPS {
            let mut vdso_tloc: time_t = 0;
            let mut libc_tloc: time_t = 0;
            let vdso_time = vdso.time(&mut vdso_tloc as *mut _).unwrap();
            let libc_time = unsafe { libc::time(&mut libc_tloc as *mut _) };
            println!(
                "[time()] vdso: {}, libc: {}, diff: {}",
                vdso_time,
                libc_time,
                libc_time - vdso_time
            );
            assert_eq!(vdso_time, libc_time);
            assert_eq!(vdso_time, vdso_tloc);

            let ten_millis = time::Duration::from_millis(SLEEP_DURATION);
            thread::sleep(ten_millis);
        }
    }

    #[test]
    fn test_clock_gettime_realtime() {
        test_single_clock_gettime(CLOCK_REALTIME);
    }

    #[test]
    fn test_clock_gettime_realtime_coarse() {
        test_single_clock_gettime(CLOCK_REALTIME_COARSE);
    }

    #[test]
    fn test_clock_gettime_monotonic() {
        test_single_clock_gettime(CLOCK_MONOTONIC);
    }

    #[test]
    fn test_clock_gettime_monotonic_coarse() {
        test_single_clock_gettime(CLOCK_MONOTONIC_COARSE);
    }

    #[test]
    fn test_clock_gettime_monotonic_raw() {
        test_single_clock_gettime(CLOCK_MONOTONIC_RAW);
    }

    #[test]
    fn test_clock_gettime_boottime() {
        test_single_clock_gettime(CLOCK_BOOTTIME);
    }

    fn test_single_clock_gettime(clockid: clockid_t) {
        let vdso = Vdso::new().unwrap();
        for _ in 0..LOOPS {
            let mut vdso_tp = timespec {
                tv_sec: 0,
                tv_nsec: 0,
            };
            let mut libc_tp = timespec {
                tv_sec: 0,
                tv_nsec: 0,
            };

            vdso.clock_gettime(clockid, &mut vdso_tp).unwrap();

            unsafe { libc::clock_gettime(clockid as _, &mut libc_tp as *mut _) };

            let diff = (libc_tp.tv_sec - vdso_tp.tv_sec) * NSEC_PER_SEC as i64
                + (libc_tp.tv_nsec - vdso_tp.tv_nsec);

            println!(
                "[clock_gettime({:?})], vdso: [ tv_sec {}, tv_nsec {} ], libc: [ tv_sec {}, tv_nsec {} ], diff: {} nsec",
                clockid, vdso_tp.tv_sec, vdso_tp.tv_nsec, libc_tp.tv_sec, libc_tp.tv_nsec, diff,
            );
            assert!(diff >= 0 && diff <= MAX_DIFF_NSEC as i64);

            let ten_millis = time::Duration::from_millis(SLEEP_DURATION);
            thread::sleep(ten_millis);
        }
    }

    #[test]
    fn test_gettimeofday() {
        let vdso = Vdso::new().unwrap();
        for _ in 0..LOOPS {
            let mut vdso_tv = timeval {
                tv_sec: 0,
                tv_usec: 0,
            };
            let mut vdso_tz = timezone::default();
            let mut libc_tv = timeval {
                tv_sec: 0,
                tv_usec: 0,
            };
            let mut libc_tz = timezone::default();

            vdso.gettimeofday(&mut vdso_tv as *mut _, &mut vdso_tz as *mut _)
                .unwrap();

            unsafe {
                libc::gettimeofday(
                    &mut libc_tv as *mut _,
                    &mut libc_tz as *mut timezone as *mut _,
                )
            };

            let diff = (libc_tv.tv_sec - vdso_tv.tv_sec) * USEC_PER_SEC as i64
                + (libc_tv.tv_usec - vdso_tv.tv_usec);

            println!(
                "[gettimeofday()], vdso: [ tv_sec {}, tv_usec {} ], libc: [ tv_sec {}, tv_usec {} ], diff: {} nsec",
                vdso_tv.tv_sec, vdso_tv.tv_usec, libc_tv.tv_sec, libc_tv.tv_usec, diff,
            );
            assert!(diff >= 0 && diff <= (MAX_DIFF_NSEC / NSEC_PER_USEC) as i64);
            assert_eq!(vdso_tz.tz_minuteswest, libc_tz.tz_minuteswest);
            assert_eq!(vdso_tz.tz_dsttime, libc_tz.tz_dsttime);

            let ten_millis = time::Duration::from_millis(SLEEP_DURATION);
            thread::sleep(ten_millis);
        }
    }

    #[test]
    fn test_clock_getres() {
        test_single_clock_getres(CLOCK_REALTIME_COARSE);
        test_single_clock_getres(CLOCK_MONOTONIC_COARSE);
        test_single_clock_getres(CLOCK_REALTIME);
        test_single_clock_getres(CLOCK_MONOTONIC);
        test_single_clock_getres(CLOCK_BOOTTIME);
        test_single_clock_getres(CLOCK_MONOTONIC_RAW);
    }

    fn test_single_clock_getres(clockid: clockid_t) {
        let vdso = Vdso::new().unwrap();
        for _ in 0..LOOPS {
            let mut vdso_tp = timespec {
                tv_sec: 0,
                tv_nsec: 0,
            };
            let mut libc_tp = timespec {
                tv_sec: 0,
                tv_nsec: 0,
            };

            vdso.clock_getres(clockid, &mut vdso_tp).unwrap();

            unsafe { libc::clock_getres(clockid as _, &mut libc_tp as *mut _) };

            println!(
                "[clock_getres({:?})], vdso: [ tv_sec {}, tv_nsec {} ], libc: [ tv_sec {}, tv_nsec {} ]",
                clockid, vdso_tp.tv_sec, vdso_tp.tv_nsec, libc_tp.tv_sec, libc_tp.tv_nsec,
            );
            assert_eq!(vdso_tp.tv_sec, libc_tp.tv_sec);
            assert_eq!(vdso_tp.tv_nsec, libc_tp.tv_nsec);
        }
    }
}
