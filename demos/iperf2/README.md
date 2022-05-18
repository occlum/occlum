# Run iperf2 on Occlum

[`Iperf2`](https://sourceforge.net/projects/iperf2/) is a popular tool for measuring Internet bandwidth performance. 

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

* Start the iperf2 server
```
cd occlum_server
occlum run /bin/iperf -s -p 6888
```

* Start the iperf2 client with 16 threads
```
cd occlum_client
occlum run /bin/iperf -c 127.0.0.1 -p 6888 -P 16
```
