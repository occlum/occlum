use std::time::Duration;
use vdso_time::{ClockId, clock_getres, clock_gettime};

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

fn benchmark(name: &str, func: impl Fn() -> Duration) {
    let start = clock_gettime(ClockId::CLOCK_MONOTONIC).unwrap();
    let loops = 1000000;
    for _ in 0..loops {
        black_box(func());
    }
    let end = clock_gettime(ClockId::CLOCK_MONOTONIC).unwrap();
    println!("[{}] avg_time: {:?} ns", name, (end - start).as_nanos() / loops);
}

fn vdso_benchmarks() {
    benchmark("vdso clock_gettime()", || clock_gettime(ClockId::CLOCK_MONOTONIC).unwrap());
    benchmark("vdso clock_getres()", || clock_getres(ClockId::CLOCK_MONOTONIC).unwrap());
}