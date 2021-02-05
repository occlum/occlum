# async-socket

async socket IO.

## Usage
To use async-socket, place the following line under the `[dependencies]` section in your `Cargo.toml`:

```
async-socket = { path = "your_path/async-socket" }
```

if use async-socket in SGX (based on rust-sgx-sdk), place the following line under the `[dependencies]` section in your `Cargo.toml` and prepare incubator-teaclave-sgx-sdk envirenments according to async-socket's `Cargo.toml`:
```
async-socket = { path = "your_path/async-socket", features = ["sgx"]  }
```