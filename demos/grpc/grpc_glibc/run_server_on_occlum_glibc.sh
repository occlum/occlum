#!/bin/bash
set -e

if [ ! -d "occlum_server_instance" ];then
    mkdir occlum_server_instance
    cd occlum_server_instance
    occlum init

    rm -rf image
    copy_bom -f ../grpc_server_glibc.yaml --root image --include-dir /opt/occlum/etc/template
    occlum build
else
    cd occlum_server_instance
fi

occlum run /bin/greeter_server
