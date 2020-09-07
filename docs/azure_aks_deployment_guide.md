# Quick Start: Deploy Occlum on Azure AKS

[Azure Kubernetes Service (AKS)](https://azure.microsoft.com/en-us/services/kubernetes-service/#getting-started) is a fully-managed Kubernetes service, which cuts off the complexity of managing Kubernetes to let users only focus on the agent nodes within the clusters. With the support of [Azure Confidential Computing](https://docs.microsoft.com/en-us/azure/confidential-computing/overview), SGX containers can be easily deployed as Kubernetes pods.

This document provides instructions on how to deploy Occlum-based SGX containers on AKS.

# Prerequisites: AKS with Confidential Computing Nodes

Please follow [this guide](https://aka.ms/accakspreview) first to deploy AKS with confidential computing nodes. Please be noted that these nodes have already installed the SGX DCAP driver and Intel FSGSBASE enablement patch.

## Run a Sample Open Enclave App
To make sure the cluster nodes are correctly configured, let's run a test with below `sample.yml` which will use the container image from [Open Enclave](https://github.com/openenclave/openenclave) CI team:
```yaml
# sample.yml
apiVersion: batch/v1
kind: Job
metadata:
  name: sgx-test
  labels:
    app: sgx-test
spec:
  template:
    metadata:
      labels:
        app: sgx-test
    spec:
      containers:
      - name: sgxtest
        image: oeciteam/sgx-test:1.0
        resources:
          limits:
            kubernetes.azure.com/sgx_epc_mem_in_MiB: 10
      restartPolicy: Never
  backoffLimit: 0
```

Then run commands in bash:
```
kubectl apply -f sample.yml
kubectl logs -l app=sgx-test
```

If the cluster is correctly configured, the log should be like this:
```
Hello world from the enclave
Enclave called into host to print: Hello World!
```

## Run an Occlum Sample App
Now you can deploy Occlum as you normally do with AKS application deployment:

**1. Create `hello_world.yml`**
```yaml
# hello_world.yml
apiVersion: v1
kind: Pod
metadata:
  name: occlum-hello
spec:
  tolerations:
  - key: kubernetes.azure.com/sgx_epc_mem_in_MiB
    operator: Exists
    effect: NoSchedule
  containers:
  - name: occlum-test
    image: occlum/occlum:0.15.1-ubuntu18.04
    command: ["/bin/bash"]
    args:
      - -c
      - >-
          cd /root/demos/hello_c;
          make;
          occlum new instance;
          cp hello_world instance/image/bin;
          cd instance && occlum build;
          while [ true ];do occlum run /bin/hello_world;sleep 3;done;
    resources:
      limits:
        kubernetes.azure.com/sgx_epc_mem_in_MiB: 10
```
**2. Deploy hello world test**
```shell
kubectl apply -f hello_world.yml
kubectl get pods
```

You can see the pod `occlum-hello`. And then check the log of the pod:
```shell
kubectl logs occlum-hello
```

You should see logs of Occlum build and "Hello World" printed out constantly.

## Deploy a Go Web Server
Occlum supports applications written in most of the mainstream programming languages, including C/C++, Java, Python, Go and Rust. Users can easily deploy a Go web server with the provided [Go demo](demos/golang/README.md). To run Go web server on AKS, please follow below steps:

**1. Create `go_web_server.yml`**
```yaml
# go_web_server.yml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: occlum-go-server
spec:
  selector:
    matchLabels:
      app: occlum-go-server
  replicas: 3
  template:
    metadata:
      labels:
        app: occlum-go-server
    spec:
      tolerations:
      - key: kubernetes.azure.com/sgx_epc_mem_in_MiB
        operator: Exists
        effect: NoSchedule
      containers:
      - name: occlum-go-server
        image: occlum/occlum:0.14.0-ubuntu18.04
        ports:
        - containerPort: 8090
        command: ["/bin/bash"]
        args:
          - -c
          - >-
              occlum-go get -u -v github.com/gin-gonic/gin;
              cd /root/demos/golang;
              occlum-go build -o web_server -buildmode=pie ./web_server.go;
              ./run_golang_on_occlum.sh;
        resources:
          limits:
            kubernetes.azure.com/sgx_epc_mem_in_MiB: 10
```
Please be noted that Go web server needs much more EPCs than a simple hello world. The `kubernetes.azure.com/sgx_epc_mem_in_MiB` can be enlarged based on the resources you have. Technically, the more EPC configured, the faster the web server runs. You can always add more [Azure Confidential Computing nodes](https://docs.microsoft.com/en-us/azure/virtual-machines/dcv2-series) to your clusters if you need more EPCs or other resources.

**2. Deploy the Web Server**
```shell
kubectl apply -f go_web_server.yml
```

And the server is ready when you can see:
```
[GIN-debug] GET    /ping                     --> main.main.func1 (3 handlers)
[GIN-debug] Listening and serving HTTP on :8090
```
from the pod's log.

**3. Expose the Deployment**
Create a Service object that exposes the deployment:
```shell
kubectl expose deployment occlum-go-server --type=LoadBalancer --name=occlum-go-web-service
```
Display information about the Service:
```shell
kubectl get services occlum-go-web-service
```
And you should see `EXTERNAL-IP` field which is assigned a public IP address and `PORT(S)` field which lists the port we specified in `go_web_server.yml`.

**4. Send a Request**
Open a web browser and visit: `http://<EXTERNAL-IP showed in step-3>:8090/ping` or just run in a new terminal:
```shell
curl http://<EXTERNAL-IP showed in step-3>:8090/ping
```
And you should see `{"message":"pong"}`.

## Attestation
Enclave attestation is a process to verify that an enclave is secure and trustworthy. Azure also provides Azure Attestation for customers to have end to end protection. Please visit [this site](https://aka.ms/azureattestation) for more details.
