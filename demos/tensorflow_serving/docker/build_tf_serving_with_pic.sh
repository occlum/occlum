#!/bin/bash
set -e

if  [ ! -n "$1" ] ; then
    tag=latest
else
    tag=$1
fi

# You can remove build-arg http_proxy and https_proxy if your network doesn't need it
proxy_server="" # your http proxy server

DOCKER_BUILDKIT=0 docker build \
    -f Dockerfile.devel . \
    -t tf_serving_pic:${tag} \
    --build-arg http_proxy=${proxy_server} \
    --build-arg https_proxy=${proxy_server} \
    --build-arg no_proxy=localhost,127.0.0.0/1 \
