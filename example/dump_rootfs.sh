#!/bin/bash
set -e

script_dir="$( cd "$( dirname "${BASH_SOURCE[0]}"  )" >/dev/null 2>&1 && pwd )"

tag="latest"
dest=${script_dir}

function usage {
    cat << EOM
Dump rootfs content from the specified container image.
usage: $(basename "$0") [OPTION]...
    -i <container image name> the container image name
    -g <tag> container image tag
    -d <destination> the directiory to put dumped rootfs  
    -h <usage> usage help
EOM
    exit 0
}

function process_args {
    while getopts ":i:g:d:h" option; do
        case "${option}" in
            i) container=${OPTARG};;
            g) tag=${OPTARG};;
            d) dest=${OPTARG};;
            h) usage;;
        esac
    done

    if [[ "${container}" == "" ]]; then
        echo "Error: Please specify the container image -i <container image name>."
        exit 1
    fi
}

process_args "$@"

rm -rf rootfs.tar
docker export $(docker create --network host --name rootfs_dump ${container}:${tag}) -o rootfs.tar
docker rm rootfs_dump

rm -rf ${dest}/rootfs && mkdir -p ${dest}/rootfs
tar xf rootfs.tar -C ${dest}/rootfs

echo "Successfully dumped ${container}:${tag} rootfs to ${dest}/rootfs."
