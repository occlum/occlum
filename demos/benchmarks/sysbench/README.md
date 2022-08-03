# Run Linux sysbench

In this demo, we will show how to run the Linux sysbench inside Occlum.

Linux [`sysbench`](https://github.com/akopytov/sysbench) is a scriptable multi-threaded benchmark tool.
Occlum could also run the `sysbench` for CPU/Threads/Memory/Mutex/... benchmarks.

Please note it is configured with "--without-mysql", so no mysql database benchmark can be done.

## Download and build the Linux sysbench from source code
```
./dl_and_build.sh
```

## Prepare the Occlum instance for sysbench demo
```
./prepare_sysbench.sh
```

## Run the sysbench demo

For example,

* CPU benchmark
```
occlum/demos/sysbench/occlum_instance# occlum run /bin/sysbench cpu  --cpu-max-prime=2000 --threads=2 run
```

* threads benchmark
```
# occlum/demos/sysbench/occlum_instance# occlum run /bin/sysbench threads --threads=200 --thread-yields=100 --thread-locks=4 --time=10 run
```

More test commands could refer to
```
# occlum/demos/sysbench/occlum_instance# occlum run /bin/sysbench --help
```


