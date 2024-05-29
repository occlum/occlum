#!/bin/bash
set -e

script_dir="$( cd "$( dirname "${BASH_SOURCE[0]}"  )" >/dev/null 2>&1 && pwd )"
top_dir=$(dirname "${script_dir}")

registry="demo"
tag="latest"

function usage {
    cat << EOM
Build Occlum Flink container images for k8s deployment.
usage: $(basename "$0") [OPTION]...
    -r <container image registry> the container image registry
    -g <tag> container image tag
    -h <usage> usage help
EOM
    exit 0
}

function process_args {
    while getopts ":r:g:h" option; do
        case "${option}" in
            r) registry=${OPTARG};;
            g) tag=${OPTARG};;
            h) usage;;
        esac
    done
}

process_args "$@"

echo ""
echo "############################"
echo "Build Occlum Flink container image for k8s deployment"
echo "  Container images registry: ${registry}"
echo "  Container images tag: ${tag}"
echo ""

pushd ${top_dir}
echo "Install openjdk 11 first ..."
./preinstall_deps.sh

echo "Download Flink ..."
./download_flink.sh
cp ./kubernetes/flink-console.sh ./flink-1.15.2/bin/

echo "Build Occlum instance ..."
./build_occlum_instance.sh k8s

echo ""
echo "Build Occlum container image ..."

docker build \
    -f Dockerfile \
    -t ${registry}/occlum_flink:${tag} .

echo "Build is done"

popd
