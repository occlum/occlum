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
use vdso_time::ClockId;

let time = vdso_time::clock_gettime(ClockId::CLOCK_MONOTONIC).unwrap();
println!("vdso_time::clock_gettime: {:?}", time);

let res = vdso_time::clock_getres(ClockId::CLOCK_MONOTONIC).unwrap();
println!("vdso_time::clock_getres: {:?}", res);
```

```
use vdso_time::{Vdso, ClockId};

let vdso = Vdso::new().unwrap();

let time = vdso.clock_gettime(ClockId::CLOCK_MONOTONIC).unwrap();
println!("vdso.clock_gettime: {:?}", time);

let res = vdso.clock_getres(ClockId::CLOCK_MONOTONIC).unwrap();
println!("vdso.clock_getres: {:?}", res);
}
```