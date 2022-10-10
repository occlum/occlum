# gRPC Package With RA-TLS

## Simple GRPC protocol for the demo

* Server side, holds a [`json file`](./secret_config.json) including secret name and the secret's base64 encoded string.

* Client side, request the secret by the secret name.

## Example libraries/executables in the demo

* libhw_grpc_proto.so
* libgrpc_ratls_client.so
* libgrpc_ratls_server.so
* client
* server

### APIs defined for sample server and client

* Server
```
int grpc_ratls_start_server(
    const char *server_addr, // grpc server address+port, such as "localhost:50051"
    const char *config_json, // ratls handshake config json file
    const char *secret_json  // secret config json file
);
```

* Client
```
int grpc_ratls_get_secret(
    const char *server_addr, // grpc server address+port, such as "localhost:50051"
    const char *config_json, // ratls handshake config json file
    const char *name, // secret name to be requested
    const char *secret_file // secret file to be saved
);
```

All source could be found on [`example`](./grpc/v1.38.1/examples/cpp/ratls/)


## Executing the demo in Occlum

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

The following command will generate the client and server occlum images. It automatically parses the mr_enclave and mr_signer of the client, and write the value into dynamic_config.json. If you want to verify the other measurements of client, please modify the `ra_config_template.json` before run the script.
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
./run.sh client <request_secret_name> ( cert, key )
```

***Note:*** 1. The demo runs in the same machine by default. If you want to run server and client in different machines. Please modify the examples/cpp/ratls.
            2. If you want to test in your local network with your own PCCS server, you need to modify the /etc/sgx_default_qcnl.conf

