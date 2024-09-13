# Tensorflow Serving Demo

This demo is different from the demo [tensorflow-serving](../tensorflow_serving/) which demostrate the way to build TF from source. This demo just dumps the binary **tensorflow_model_server** from docker image and then uses it in the Occlum instance. It is a basically simplified version of the [example](https://github.com/occlum/occlum/tree/master/example). Please note that this way could only work on Ubuntu based Occlum. For other OS, either building TF from source, or dumping the TF binary from that OS based **tensorflow-serving** docker image and do following in that OS based Occlum docker image.

## How-to build

First, please make sure `docker` is installed successfully in your host.
And then you can dump the binary **tensorflow_model_server** from docker image `tensorflow/serving`.

```
# ./prepare_tf_and_model.sh
```

Start the Occlum container per the [doc](https://occlum.readthedocs.io/en/latest/quickstart.html#start-the-occlum-dev-container).

All the following are running in the above container.

The following command will generate the TLS key and certificates for localhost( server domain name). The server.crt will be used by client. The ssl.cfg is used by TF serving.
```
./generate_ssl_config.sh localhost
```

Now build TF serving Occlum instance.

```
# ./build_occlum_instance.sh
```

Last, build the client for TF serving test.
There is an example python based [`inference client`](./client/resnet_client_grpc.py) which sends a picture to tensorflow serving service to do inference with previously generated server certificate.

Install the dependent python packages.
```
# pip3 install -r client/requirements.txt
```

## How-to run

### Start the tensorflow serving

Script [`run.sh`](./run.sh) is provided to start the TF serving service in the Occlum.

```
# ./run.sh
```

The tensorflow serving service would be available by GRPC secure channel `localhost:9000`.

### Try the inference request

Start the inference request.
```
# cd client
# python3 resnet_client_grpc.py --server=localhost:9000 --crt ../ssl_configure/server.crt --image cat.jpg
```

If everything goes well, you will get the most likely predication class (int value, mapping could be found on https://storage.googleapis.com/download.tensorflow.org/data/ImageNetLabels.txt) and its probability.

### Benchmark

Below command can do benchmark test.

```
# cd client
# python3 benchmark.py --server localhost:9000 --crt ../ssl_configure/server.crt --cnum 4 --loop 10 --image cat.jpg
```
