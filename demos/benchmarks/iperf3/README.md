# Run iperf3 on Occlum

[`Iperf3`](https://github.com/esnet/iperf) is a popular tool for measuring Internet bandwidth performance.

### Build
```
./build.sh
```

If everything goes well, it generates two occlum instances.
```
occlum_server
occlum_client
```

### Run the test

* Start the iperf3 server for on one time benchmark
```
cd occlum_server
occlum run /bin/iperf3 -s -p 6777 -1
```

* Start the iperf3 client with 16 streams
```
cd occlum_client
occlum run /bin/iperf3 -c 127.0.0.1 -p 6777 -P 16
```
