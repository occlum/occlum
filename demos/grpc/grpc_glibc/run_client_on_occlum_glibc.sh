#!/bin/bash
set -e

if [ ! -d "occlum_client_instance" ];then
    mkdir occlum_client_instance
    cd occlum_client_instance
    occlum init

    rm -rf image
    copy_bom -f ../grpc_client_glibc.yaml --root image --include-dir /opt/occlum/etc/template
    occlum build
else
    cd occlum_client_instance
fi

apt update && apt install -y netcat

while ! nc -z 127.0.0.1 50051; do
    sleep 1
done

echo "greeter_client is running"
occlum run /bin/greeter_client
