include!("common/bench.rs");

fn linux_time() -> u64 {
    let mut tloc: time_t = 0;
    unsafe {
        libc::time(&mut tloc as *mut _);
    }
    tloc as u64
}

fn linux_gettimeofday() -> u64 {
    let mut tv = timeval {
        tv_sec: 0,
        tv_usec: 0,
    };
    unsafe {
        libc::gettimeofday(&mut tv as *mut _, std::ptr::null_mut());
    }
    tv.tv_sec as u64 * 1000000 + tv.tv_usec as u64
}

fn linux_clock_gettime() -> u64 {
    let mut tp = timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    let clockid = CLOCK_REALTIME;
    unsafe {
        libc::clock_gettime(clockid, &mut tp as *mut _);
    }
    tp.tv_sec as u64 * 1000000000 + tp.tv_nsec as u64
}

fn linux_clock_getres() -> u64 {
    let mut tp = timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    let clockid = CLOCK_REALTIME;
    unsafe {
        libc::clock_getres(clockid, &mut tp as *mut _);
    }
    tp.tv_sec as u64 * 1000000000 + tp.tv_nsec as u64
}

fn linux_benchmarks() {
    benchmark("Linux time()", linux_time);
    benchmark("Linux gettimeofday()", linux_gettimeofday);
    benchmark("Linux clock_gettime()", linux_clock_gettime);
    benchmark("Linux clock_getres()", linux_clock_getres);
}

fn main() {
    linux_benchmarks();
    vdso_benchmarks();
}
