# Use Mongoose HTTPS file server with Occlum

This project demonstrates how to run a HTTPS file server with [Mongoose Embedded Web Server Library](https://github.com/cesanta/mongoose).

Step 1: Download and build Mongoose and OpenSSL, then build the sample HTTPS file server shipped with Mongoose's source code
```
./download_and_build_mongoose.sh
```
When completed, the resulting file server can be found at `./mongoose_src/examples/simplest_web_server_ssl/simplest_web_server_ssl`.

Step 2: You can run the HTTPS file server either on Occlum
```
./run_https_server_in_occlum.sh
```
or on Linux
```
./run_https_server_in_linux.sh
```
The HTTPS file server should now start to listen on port 8443 and serve HTTPS requests.

Step 3: To check whether the HTTPS server works, run
```
curl -k https://127.0.0.1:8443
```
in another terminal.

It is also possible to access the HTTPS server directly in a Web browser. But if you are testing in a Docker container, you won't be able to open the URL `https://127.0.0.1:8443` in a browser on the host OS. To fix this, you have to manually map port 8843 of the Docker container to a port on the host OS. Check out how to use [the `-p` argument of `docker run` command](https://docs.docker.com/engine/reference/commandline/run/).
