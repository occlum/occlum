# Run NGINX with Occlum

This project demonstrates how Occlum enables [NGINX](https://nginx.org/en/) in SGX enclaves.

Step 1: Download NGINX source code and build the NGINX executable
```
./prepare_nginx.sh
```
Once completed, the resulting NGINX source code can be found in the `source_code` directory.

Step 2: Run NGINX server
```
./run_occlum_nginx_server.sh
```

Step 3: In another terminal, run a `curl` command
```
curl -v http://localhost:80
```
