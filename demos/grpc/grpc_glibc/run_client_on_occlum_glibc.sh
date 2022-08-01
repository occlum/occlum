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

occlum run /bin/greeter_client
