# vdso-time
A rust crate for getting time using vDSO. This crate can support host and SGX (based on Rust-SGX-SDK).

## Getting Started
Add the following dependency to your Cargo manifest:

```
vdso-time = { path = "yourpath/vdso-time" }
```

If you want to use in SGX environment, add the following dependency to your Cargo manifest:

```
vdso-time = { path = "yourpath/vdso-time", default-features = false, features = ["sgx"] }
```

## API examples

```
use vdso_time::{time_t, timespec, timeval, timezone, Vdso, CLOCK_REALTIME};

// init vdso
let vdso = Vdso::new().unwrap();

// time()
let mut tloc: time_t = 0;
let time = vdso.time(&mut tloc as *mut _).unwrap();
println!("time(): t {}, tloc {}", time, tloc);

// gettimeofday()
let mut tv = timeval {
    tv_sec: 0,
    tv_usec: 0,
};
let mut tz = timezone::default();
vdso.gettimeofday(&mut tv as *mut _, &mut tz as *mut _)
    .unwrap();
println!(
    "gettimeofday(): tv_sec {}, tv_usec {}; tz_minuteswest {}, tz_dsttime {}",
    tv.tv_sec, tv.tv_usec, tz.tz_minuteswest, tz.tz_dsttime,
);

// clock_gettime
let mut tp = timespec {
    tv_sec: 0,
    tv_nsec: 0,
};
let clockid = CLOCK_REALTIME;
vdso.clock_gettime(clockid, &mut tp).unwrap();
println!(
    "clock_gettime({:?}): tv_sec {}, tv_nsec {}",
    clockid, tp.tv_sec, tp.tv_nsec
);

// clock_getres
let mut tp = timespec {
    tv_sec: 0,
    tv_nsec: 0,
};
let clockid = CLOCK_REALTIME;
vdso.clock_getres(clockid, &mut tp).unwrap();
println!(
    "clock_getres({:?}): tv_sec {}, tv_nsec {}",
    clockid, tp.tv_sec, tp.tv_nsec
);
```