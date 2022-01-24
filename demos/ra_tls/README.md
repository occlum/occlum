# gRPC Package With RA-TLS


#### Executing the demo in Occlum

The following command will download prerequisite source and the gRPC source code.
```
./download_and_prepare.sh
```

The following command will patch the gRPC source code and do the build and install.
```
./build_and_install.sh
```

If musl-libc version is expected.
```
./build_and_install.sh musl
```

The following command will generate the client and server occlum images. It automatically parses the mr_enclave and mr_signer of the client, and write the value into dynamic_config.json. If you want to verify the other measurements of client, please modify the dynamic_config.json before run the script.
```
./build_occlum_instance.sh
```
If previous build choice is `musl`.
```
./build_occlum_instance.sh musl
```

Run the gRPC server & client in occlum.

```
./run.sh server &
./run.sh client
```

***Note:*** 1. The demo runs in the same machine by default. If you want to run server and client in different machines. Please modify the examples/cpp/ratls.
            2. If you want to test in your local network with your own PCCS server, you need to modify the /etc/sgx_default_qcnl.conf

