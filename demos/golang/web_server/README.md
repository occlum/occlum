# Use Golang with Occlum

This project demonstrates how Occlum enables [Golang](https://golang.org) programs running in SGX enclaves, the demo program is a HTTP web server based on a widely used web framework([Gin](https://gin-gonic.com)) for Go.

Step 1: Install Gin with `occlum-go`, it may take a few minutes
```
occlum-go mod init web_server && \
occlum-go get -u -v github.com/gin-gonic/gin
```

Step 2: Build the Golang web server using the Occlum Golang toolchain(i.e., `occlum-go`)
```
occlum-go build -o web_server ./web_server.go
```

Step 3: You can run the web server demo on Occlum via
```
./run_golang_on_occlum.sh
```
The HTTP web server should now start to listen on port 8090 and serve HTTP requests.

Step 4: To check whether the HTTP server works, run
```
curl http://127.0.0.1:8090/ping
```
in another terminal, and get the response `{"message":"pong"}`.
