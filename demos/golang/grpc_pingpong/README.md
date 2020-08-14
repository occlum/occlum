# Use Golang and gRPC with Occlum

This project demonstrates how Occlum enables [Golang](https://golang.org) programs with [gRPC](google.golang.org/grpc) calls running in SGX enclaves. The client program invokes a gRPC call with a ping message, and receives a pong message sent by the server program.

Step 1: Build the Golang gRPC application using the Occlum Golang toolchain via
```
./prepare_ping_pong.sh
```

Step 2: Run the gRPC server demo on Occlum via
```
./run_pong_on_occlum.sh
```
The gRPC server should now start to listen on port 8888 and serve incoming requests.

Step 3: Run the gRPC client demo on Occlum via
```
./run_ping_on_occlum.sh
```
After the reply message is received, the latency incurred during a gRPC call will be printed out.
