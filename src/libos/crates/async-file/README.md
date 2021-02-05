# async-file

async file IO.

## Usage
To use async-file, place the following line under the `[dependencies]` section in your `Cargo.toml`:

```
async-file = { path = "your_path/async-file" }
```

if use async-file in SGX (based on rust-sgx-sdk), place the following line under the `[dependencies]` section in your `Cargo.toml` and prepare incubator-teaclave-sgx-sdk envirenments according to async-file's `Cargo.toml`:
```
async-file = { path = "your_path/async-file", features = ["sgx"]  }
```