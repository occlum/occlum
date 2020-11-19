#!/bin/bash
script_dir="$( cd "$( dirname "${BASH_SOURCE[0]}"  )" >/dev/null 2>&1 && pwd )"

report_error() {
    RED=$(tput setaf 1)
    NO_COLOR=$(tput sgr0)

    cat <<EOF
${RED}error:${NO_COLOR} input is invalid

build_image
Build an Occlum Docker image for a specific OS

USAGE:
    build_image.sh <OCCLUM_LABEL> <OS_NAME>

<OCCLUM_LABEL>:
    An arbitrary string chosen by the user to describe the version of Occlum preinstalled in the Docker image, e.g., "latest", "0.12.0", "prerelease", and etc.

<OS_NAME>:
    The name of the OS distribution that the Docker image is based on. Currently, <OS_NAME> must be one of the following values:
        ubuntu18.04         Use Ubuntu 18.04 as the base image
        centos7.5           Use CentOS 7.5 as the base image
        centos8.1           Use CentOS 8.1 as the base image

The resulting Docker image will have "occlum/occlum:<OCCLUM_LABEL>-<OS_NAME>" as its label.
EOF
    exit 1
}

set -e

if [[ ( "$#" < 2 ) ]] ; then
    report_error
fi

occlum_branch=master
occlum_label=$1
os_name=$2

function check_item_in_list() {
    item=$1
    list=$2
    [[ $list =~ (^|[[:space:]])$item($|[[:space:]]) ]]
}

check_item_in_list "$os_name" "ubuntu18.04 centos7.5 centos8.1" || report_error

cd "$script_dir/.."
docker build -f "$script_dir/Dockerfile.$os_name" -t "occlum/occlum:$occlum_label-$os_name" --build-arg OCCLUM_BRANCH=$occlum_branch .
