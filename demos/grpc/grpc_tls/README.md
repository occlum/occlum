# Run gRPC TLS C++ Client/Server on Occlum

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
Prepare the gRPC TLS C++ Hello World demo Occlum instance, which consists of a client and server:
```
./prepare_occlum_instance.sh
```
Then you can see two occlum instance created for server(`occlum_server`) and client(`occlum_client`).

## Step 4:
Start `tls server` which will listen on port `50051` on occlum:
```
cd occlum_server
occlum run /bin/greeter_secure_server
```

Then you can invoke gRPC service by running `tls client` in a different terminal on occlum:
```
cd occlum_client
occlum run /bin/greeter_secure_client
```

And you will see the "Greeter received: Hello world" in the client side output.
