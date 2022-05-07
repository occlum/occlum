# Modified Spark running on Occlum #

[Spark](https://github.com/apache/spark) is a unified analytics engine for large-scale data processing, which is widely used in big data situation. However, many of processing information in spark is sensitive. 

Here, Occlum is a memory-safe, multi-process library OS (LibOS) for [Intel SGX](https://software.intel.com/en-us/sgx), it enables Trusted Execution Environment for data processing and storage in spark. This example demonstrates how to use spark protected by Occlum. We presented a simple but representive example, Spark Pi. 

Note in the example, all components are run on single machine within one container. Besides, the modified Spark is provided by [Analytics Zoo](https://github.com/intel-analytics/analytics-zoo) team, thanks for their enormous contribution.

## How-to Run

First, please make sure `docker` is installed successfully in your host. Then start the Occlum container (use version `0.27.2-ubuntu20.04` for example) as below.

```
$ sudo docker run -it --privileged --name=spark_demo -v /dev/sgx_enclave:/dev/sgx/enclave -v /dev/sgx_provision:/dev/sgx/provision -v $(which docker):/usr/bin/docker -v /var/run/docker.sock:/var/run/docker.sock occlum/occlum:0.27.2-ubuntu20.04
```

Since installation procedures require apply `docker run in docker` method, `/usr/bin/docker` and `/var/run/docker.sock` need to be mounted in container.

All the following are running in the above container.

### Build all the content

Firstly, install openjdk 11 and copy modified spark from analytics zoo image.

```
./prepare_env.sh
```

Secondly, run Spark Pi in Occlum.

```
./run_spark.sh pi
```

