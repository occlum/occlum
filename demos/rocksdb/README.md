## Run RocksDB on Occlum

[RocksDB](https://github.com/facebook/rocksdb) is a high performance database for key-value data. This demo demonstrates how can RocksDB run on Occlum, guarded with Intel SGX.

### Step 1: Preinstall dependencies
Related dependencies: libgflags-dev libsnappy-dev zlib1g-dev libbz2-dev liblz4-dev libzstd-dev
```
./preinstall_deps.sh
```

### Step 2: Build RocksDB from source
```
cd demos/rocksdb && ./dl_and_build_rocksdb.sh
```

The script will download source code, compile RocksDB into a library and compile a benchmark tool binary `db_bench`.

### Step 3: Run RocksDB examples and benchmarks
```
cd demos/rocksdb && ./run_benchmark.sh
```

The script will run examples under `rocksdb/examples` then run specific benchmark using `db_bench`.

Refer to [rocksdb/wiki/Benchmarking-tools](https://github.com/facebook/rocksdb/wiki/Benchmarking-tools) and see more information about benchmarking.
