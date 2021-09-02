include!("common/bench.rs");

fn libc_clock_gettime() -> Duration {
    let mut tp = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    unsafe {
        libc::clock_gettime(ClockId::CLOCK_MONOTONIC as _, &mut tp as *mut _);
    }
    Duration::new(tp.tv_sec as u64, tp.tv_nsec as u32)
}

fn libc_clock_getres() -> Duration {
    let mut tp = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    unsafe {
        libc::clock_getres(ClockId::CLOCK_MONOTONIC as _, &mut tp as *mut _);
    }
    Duration::new(tp.tv_sec as u64, tp.tv_nsec as u32)
}

fn libc_benchmarks() {
    benchmark("Libc clock_gettime()", libc_clock_gettime);
    benchmark("Libc clock_getres()", libc_clock_getres);
}

fn main() {
    libc_benchmarks();
    vdso_benchmarks();
}
