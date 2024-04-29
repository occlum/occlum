# io-uring-callback

io-uring with callback interface.

## Usage
To use io-uring-callback, place the following line under the `[dependencies]` section in your `Cargo.toml`:

```
io-uring-callback = { path = "your_path/io-uring-callback" }
```

if use io-uring-callback in SGX (based on rust-sgx-sdk), place the following line under the `[dependencies]` section in your `Cargo.toml` and prepare incubator-teaclave-sgx-sdk envirenments according to io-uring-callback's `Cargo.toml`:
```
io-uring-callback = { path = "your_path/io-uring-callback", features = ["sgx"]  }
```