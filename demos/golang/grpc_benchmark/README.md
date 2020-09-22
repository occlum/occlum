1. Download and build the grpc benchmark
$./download_and_build_grpc_benchmark.sh

2. Run the host benchmark
$./run_host_bench.sh

3. Run the occlum benchmark
$./run_occlum_bench.sh

4. Run the server and client in different machine. 
   Step1: Run the server application fistly
	$./run_host_server.sh
	or
	$./run_occlum_server.sh
	Note: If you are running occlum server, please wait for several minutes.
	Because the server app of occlum need more time to execute.
   Step2: Run the client app in another machine.
	Modify line 10 of run_host_client.sh ip="localhost" -> ip="your server ip"
	$./run_host_client.sh

Note: If you run the server application in a docker, you need to public the container's port to host.
For example:
$./docker run -p 50051:50051 -it IMAGE_ID

