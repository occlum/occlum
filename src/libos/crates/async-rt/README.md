# async-rt

Rust async / await runtime.

## Usage
To use async-rt, place the following line under the `[dependencies]` section in your `Cargo.toml`:

```
async-rt = { path = "your_path/async-rt" }
```

if use async-rt in SGX (based on rust-sgx-sdk), place the following line under the `[dependencies]` section in your `Cargo.toml` and prepare incubator-teaclave-sgx-sdk envirenments according to async-rt's `Cargo.toml`:
```
async-rt = { path = "your_path/async-rt", features = ["sgx"]  }
```