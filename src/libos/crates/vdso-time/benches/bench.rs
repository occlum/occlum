use criterion::{black_box, criterion_group, criterion_main, Criterion};

use lazy_static::lazy_static;
use vdso_time::{time_t, timespec, timeval, Vdso, CLOCK_REALTIME};

lazy_static! {
    static ref VDSO: Vdso = Vdso::new().unwrap();
}

fn linux_time() -> time_t {
    let mut tloc: time_t = 0;
    unsafe {
        libc::time(&mut tloc as *mut _);
    }
    tloc
}

fn vdso_time() -> time_t {
    let mut tloc: time_t = 0;
    VDSO.time(&mut tloc as *mut _).unwrap();
    tloc
}

fn linux_gettimeofday() -> timeval {
    let mut tv = timeval {
        tv_sec: 0,
        tv_usec: 0,
    };
    unsafe {
        libc::gettimeofday(&mut tv as *mut _, std::ptr::null_mut());
    }
    tv
}

fn vdso_gettimeofday() -> timeval {
    let mut tv = timeval {
        tv_sec: 0,
        tv_usec: 0,
    };
    VDSO.gettimeofday(&mut tv as *mut _, std::ptr::null_mut())
        .unwrap();
    tv
}

fn linux_clock_gettime() -> timespec {
    let mut tp = timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    let clockid = CLOCK_REALTIME;
    unsafe {
        libc::clock_gettime(clockid, &mut tp as *mut _);
    }
    tp
}

fn vdso_clock_gettime() -> timespec {
    let mut tp = timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    let clockid = CLOCK_REALTIME;
    VDSO.clock_gettime(clockid, &mut tp as *mut _).unwrap();
    tp
}

fn linux_clock_getres() -> timespec {
    let mut tp = timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    let clockid = CLOCK_REALTIME;
    unsafe {
        libc::clock_getres(clockid, &mut tp as *mut _);
    }
    tp
}

fn vdso_clock_getres() -> timespec {
    let mut tp = timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    let clockid = CLOCK_REALTIME;
    VDSO.clock_getres(clockid, &mut tp as *mut _).unwrap();
    tp
}

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("linux time", |b| b.iter(|| black_box(linux_time())));
    c.bench_function("vdso time", |b| b.iter(|| black_box(vdso_time())));
    c.bench_function("linux gettimeofday", |b| {
        b.iter(|| black_box(linux_gettimeofday()))
    });
    c.bench_function("vdso gettimeofday", |b| {
        b.iter(|| black_box(vdso_gettimeofday()))
    });
    c.bench_function("linux clock_gettime", |b| {
        b.iter(|| black_box(linux_clock_gettime()))
    });
    c.bench_function("vdso clock_gettime", |b| {
        b.iter(|| black_box(vdso_clock_gettime()))
    });
    c.bench_function("linux clock_getres", |b| {
        b.iter(|| black_box(linux_clock_getres()))
    });
    c.bench_function("vdso clock_getres", |b| {
        b.iter(|| black_box(vdso_clock_getres()))
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
