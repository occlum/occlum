# TensorFlow Serving With Occlum



TensorFlow Serving is a flexible, high-performance serving system for machine learning models, designed for production environments. This demo presents a secure End-to-End TensorFlow serving solution in Occlum.

- **Runtime security.**  Occlum uses the intel SGX to provide an enclave for running applications in encrypted memory.
- **At-Rest security.**  Model and TLS key are protected by Occlum encrypted FS.
- **Communication Security.** Use the TLS to secure the gRPC communications. 

#### Executing the Tensorflow serving in Occlum

The following command will download the Resnet50 model and convert the model format.
```
./prepare_model_and_env.sh 
```

The following command will generate the TLS key and certificates for localhost( server domain name). The server.crt will be used by client. The sever.key and ssl.cfg is used by TF serving.
```
./generate_ssl_config.sh localhost
```

Run the Tensorflow Serving in occlum.

```
./run_occlum_tf_serving.sh
```

***Note:*** The demo runs in the same machine by default. If you want to run TF serving and client in different machines. Please modify the domain name in the scripts.

#### Executing the benchmark in client

Prepare the environment for client benchmark.

```
cd client
./prepare_client_env.sh
```

Run the benchmark test in client.

```
./benchmark.sh python3 localhost:8500 ../ssl_configure/server.crt
```

