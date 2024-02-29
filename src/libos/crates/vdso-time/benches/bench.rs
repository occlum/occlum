use criterion::{black_box, criterion_group, criterion_main, Criterion};
use vdso_time::ClockId;

fn libc_clock_gettime() -> libc::timespec {
    let mut tp = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    unsafe {
        libc::clock_gettime(ClockId::CLOCK_MONOTONIC as _, &mut tp as *mut _);
    }
    tp
}

fn libc_clock_getres() -> libc::timespec {
    let mut tp = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    unsafe {
        libc::clock_getres(ClockId::CLOCK_MONOTONIC as _, &mut tp as *mut _);
    }
    tp
}

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("libc clock_gettime", |b| {
        b.iter(|| black_box(libc_clock_gettime()))
    });
    c.bench_function("vdso clock_gettime", |b| {
        b.iter(|| black_box(vdso_time::clock_gettime(ClockId::CLOCK_MONOTONIC).unwrap()))
    });
    c.bench_function("libc clock_getres", |b| {
        b.iter(|| black_box(libc_clock_getres()))
    });
    c.bench_function("vdso clock_getres", |b| {
        b.iter(|| black_box(vdso_time::clock_getres(ClockId::CLOCK_MONOTONIC).unwrap()))
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
