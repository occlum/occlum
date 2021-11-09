#!/bin/sh
echo Building tf-serving with pic
cd docker
./build_tf_serving_with_pic.sh
echo Create the tensorflow_model_server 
docker create --name extract tf_serving_pic:latest
echo Copy the tensorflow_model_server 
docker cp extract:/usr/local/bin/tensorflow_model_server ../tensorflow_model_server
docker rm -f extract
