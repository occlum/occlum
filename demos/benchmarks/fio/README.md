# Use Flexible I/O Tester in SGX with Occlum

This project demonstrates how Occlum enables the [Flexible I/O Tester(FIO)](https://github.com/axboe/fio) in SGX enclaves.

Step 1: Download and build the FIO
```
./download_and_build_fio.sh
```
When completed, the FIO program is generated in the source directory of it.

Step 2: Run the FIO program to test sequential read inside SGX enclave with Occlum
```
./run_fio_on_occlum.sh fio-seq-read.fio
```

FIO uses a configuration file to run the I/O test. We have already copied and modified some configuration files from the `examples` directory of it. Please see the files in the [configs](configs/) for the detail.
