#!/bin/bash
set -e

script_dir="$( cd "$( dirname "${BASH_SOURCE[0]}"  )" >/dev/null 2>&1 && pwd )"
top_dir=$(dirname "${script_dir}")

# pip mirror is used to accelerate the speed of python pip
pip_mirror="-i https://pypi.douban.com/simple"

registry="demo"
tag="latest"
grpc_server_domain="init-ra-server-svc"
grpc_server_port="5000"

function usage {
    cat << EOM
Build Occlum TF examples container images for k8s deployment.
usage: $(basename "$0") [OPTION]...
    -r <container image registry> the container image registry
    -g <tag> container image tag
    -d <grpc_server_domain> GPRC RA server domain
    -p <grpc_server_port> GPRC RA server port
    -h <usage> usage help
EOM
    exit 0
}

function process_args {
    while getopts ":r:g:d:p:h" option; do
        case "${option}" in
            r) registry=${OPTARG};;
            g) tag=${OPTARG};;
            d) grpc_server_domain=${OPTARG};;
            p) grpc_server_port=${OPTARG};;
            h) usage;;
        esac
    done
}

process_args "$@"

echo ""
echo "############################"
echo "Build Occlum TF examples container images for k8s deployment"
echo "  Container images registry: ${registry}"
echo "  Container images tag: ${tag}"
echo "  GRPC RA server domain: ${grpc_server_domain}"
echo "  GRPC RA server port: ${grpc_server_port}"
echo ""

pushd ${top_dir}
echo "Build Occlum instances first ..."
./build_content.sh ${grpc_server_domain} ${grpc_server_port}

echo ""
echo "Build Occlum container images ..."
./build_container_images.sh ${registry} ${tag}

echo ""
echo "Build demo client container image ..."
cp ./ssl_configure/server.crt ./client/
docker build \
    --network host \
    --build-arg http_proxy=$http_proxy \
    --build-arg https_proxy=$https_proxy \
    --build-arg pip_mirror="${pip_mirror}" \
    -f container/Dockerfile_client \
    -t ${registry}/tf_demo_client:${tag} .

echo "Build is done"

popd
