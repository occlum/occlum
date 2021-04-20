use lazy_static::lazy_static;
use vdso_time::{time_t, timespec, timeval, Vdso, CLOCK_MONOTONIC, CLOCK_REALTIME};

/// from criterion crate:
/// A function that is opaque to the optimizer, used to prevent the compiler from
/// optimizing away computations in a benchmark.
///
/// This variant is stable-compatible, but it may cause some performance overhead
/// or fail to prevent code from being eliminated.
fn black_box<T>(dummy: T) -> T {
    unsafe {
        let ret = std::ptr::read_volatile(&dummy);
        std::mem::forget(dummy);
        ret
    }
}

lazy_static! {
    static ref VDSO: Vdso = Vdso::new().unwrap();
}

fn vdso_time() -> u64 {
    let mut tloc: time_t = 0;
    VDSO.time(&mut tloc as *mut _).unwrap();
    tloc as u64
}

fn vdso_gettimeofday() -> u64 {
    let mut tv = timeval {
        tv_sec: 0,
        tv_usec: 0,
    };
    VDSO.gettimeofday(&mut tv as *mut _, std::ptr::null_mut())
        .unwrap();
    tv.tv_sec as u64 * 1000000 + tv.tv_usec as u64
}

fn vdso_clock_gettime() -> u64 {
    let mut tp = timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    let clockid = CLOCK_REALTIME;
    VDSO.clock_gettime(clockid, &mut tp as *mut _).unwrap();
    tp.tv_sec as u64 * 1000000000 + tp.tv_nsec as u64
}

fn vdso_clock_getres() -> u64 {
    let mut tp = timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    let clockid = CLOCK_REALTIME;
    VDSO.clock_getres(clockid, &mut tp as *mut _).unwrap();
    tp.tv_sec as u64 * 1000000000 + tp.tv_nsec as u64
}

fn get_time_ns() -> u64 {
    let mut tp = timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    let clockid = CLOCK_MONOTONIC;
    VDSO.clock_gettime(clockid, &mut tp as *mut _).unwrap();
    tp.tv_sec as u64 * 1000000000 + tp.tv_nsec as u64
}

fn benchmark(name: &str, func: impl Fn() -> u64) {
    let start = get_time_ns();
    let loops = 1000000;
    for _ in 0..loops {
        black_box(func());
    }
    let end = get_time_ns();
    println!("[{}] avg_time: {} ns", name, (end - start) / loops);
}

fn vdso_benchmarks() {
    benchmark("vdso time()", vdso_time);
    benchmark("vdso gettimeofday()", vdso_gettimeofday);
    benchmark("vdso clock_gettime()", vdso_clock_gettime);
    benchmark("vdso clock_getres()", vdso_clock_getres);
}