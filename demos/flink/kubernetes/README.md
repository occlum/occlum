# Deploy Flink in K8S

There are several ways to deploy Flink on Kubernetes, such as [native kubernetes deployment](https://nightlies.apache.org/flink/flink-docs-release-1.19/zh/docs/deployment/resource-providers/native_kubernetes/) and [Flink Kubernetes Operator](https://nightlies.apache.org/flink/flink-kubernetes-operator-docs-release-1.8/). This tutorial shows how to use the kubernetes operator deployment.

## Prerequisites

* A Kubernetes cluster with at least one node.
* The `kubectl` command line tool is installed and configured to connect to your Kubernetes cluster.
* The `helm` command line tool is also installed and configured to connect to your Kubernetes cluster.

### Install the Flink Kubernetes Operator

Just follow the [quick start](https://nightlies.apache.org/flink/flink-kubernetes-operator-docs-release-1.8/docs/try-flink-kubernetes-operator/quick-start/) to install the Flink Kubernetes Operator.

## Build Flink K8S docker image

First, please make sure `docker` is installed successfully in your host. Then start the Occlum container (use version `latest-ubuntu20.04` for example) as below.
```
$ sudo docker run --rm -itd --network host \
        -v $(which docker):/usr/bin/docker -v /var/run/docker.sock:/var/run/docker.sock \
        occlum/occlum:latest-ubuntu20.04
```

All the following are running in the above container.

### Build

Just run the script [build.sh](./build.sh). It builds a docker image for Flink K8S.
```bash
Build Occlum Flink container images for k8s deployment.
usage: build.sh [OPTION]...
    -r <container image registry> the container image registry
    -g <tag> container image tag
    -h <usage> usage help
```
For example, if you want to build the image named `demo/occlum-flink:0.1`, just run
```bash
$ ./build.sh -r demo -g 0.1
```

Notice, during the build process, a customized [flink-console.sh](./flink-console.sh) is used to replace the original one. Users could refer to the script for details.

Once the build is done, you can push the image for next steps -- [Deploy](#deploy).

## Deploy

Based on the original yaml files in the [github](https://github.com/apache/flink-kubernetes-operator/tree/release-1.8/examples), below customized example yaml files are provided.

* [basic.yaml](./basic.yaml)
* [basic-session-deployment-and-job.yaml](./basic-session-deployment-and-job.yaml)
* [basic-session-deployment-only. yaml](./basic-session-deployment-only.yaml)
* [basic-session-job-only.yaml](./basic-session-job-only.yaml)

They have the same meaning just like their original counterparts besides some SGX/Occlum related customization settings.
You can deploy each of them.
Just notice the **image** in the yaml file should be the one you built before.

### Examples

#### Basic Application Deployment example

This is a simple deployment defined by a minimal deployment file.
The configuration contains the following:
- Defines the job to run
- Assigns the resources available for the job
- Defines the parallelism used

To run the job submit the yaml file using kubectl:
```bash
kubectl apply -f basic.yaml
```

#### Basic Session Deployment example

This example shows how to create a basic Session Cluster and then how to submit specific jobs to this cluster if needed.

##### Without jobs 

The Flink Deployment could be created without any jobs.
In this case the Flink jobs could be created later by submitting the jobs
separately.

To create a Flink Deployment with the specific resources without any jobs run the following command:
```bash
kubectl apply -f basic-session-deployment-only.yaml
```

##### Adding jobs

If the Flink Deployment is created by `basic-session-deployment-only.yaml` new job could be added
by the following command:
```bash
kubectl apply -f basic-session-job-only.yaml
```

##### Creating Deployment and Jobs together

Alternatively the Flink Deployment and the Flink Session Job configurations can be submitted together.

To try out this run the following command:
```bash
kubectl apply -f basic-session-deployment-and-job.yaml
```
