# Use Golang with Occlum

This project demonstrates how Occlum enables [Golang](https://golang.org) programs running in SGX enclaves.

Step 1: Build Golang web server program using the Occlum Golang toolchain(i.e., `occlum-go`)
```
occlum-go build -o web_server -buildmode=pie ./web_server.go
```

Step 2: You can run the web server demo on Occlum via
```
./run_golang_on_occlum.sh
```
The HTTP web server should now start to listen on port 8090 and serve HTTP requests.

Step 3: To check whether the HTTP server works, run
```
curl http://127.0.0.1:8090/hello1
```
in another terminal, and get the response "hello,1".
