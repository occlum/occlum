# Use Golang with Occlum

This project demonstrates how Occlum enables [Golang](https://golang.org) programs running in SGX enclaves, the demo program is a HTTP web server based on a widely used web framework([Gin](https://gin-gonic.com)) for Go.

Step 1: Install Gin and build Golang web server with `occlum-go`
```
./build.sh
```

Step 2: You can run the web server demo on Occlum via
```
./run_golang_on_occlum.sh
```
The HTTP web server should now start to listen on port 8090 and serve HTTP requests.

Step 3: To check whether the HTTP server works, run
```
curl http://127.0.0.1:8090/ping
```
in another terminal, and get the response `{"message":"pong"}`.
