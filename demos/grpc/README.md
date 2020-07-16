# Run gRPC C++ Client/Server on Occlum

## Step 1:
Downlaod, build and install openssl into `/usr/local/occlum/x86_64-linux-musl/lib`:
```
./download_and_install_openssl.sh
```

## Step 2:
Download, build and install cares, protobuf and finally gRPC into `/usr/local/occlum/x86_64-linux-musl/lib`:
```
./download_and_install_grpc.sh
```

## Step 3:
Prepare the gRPC C++ Hello World sample project, which consists of a client and server:
```
./prepare_client_server.sh
```
Then you can see the source code in client and server if you want.

## Step 4:
Run the demo `server` which will listen on port `50051` on occlum:
```
./run_server_on_occlum.sh
```
or on host:
```
./run_server_on_host.sh
```
Then you can invoke gRPC service by running `client` in a different terminal on occlum:
```
./run_client_on_occlum.sh
```
or on host:
```
./run_server_on_host.sh
```
And you will see the "Greeter received: Hello world" in the client side output.
