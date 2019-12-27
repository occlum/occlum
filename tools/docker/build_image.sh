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
    An arbitrary string chosen by the user to describe the version of Occlum preinstalled in the Docker image, e.g., "latest", "0.8.0", "prerelease", and etc.

<OS_NAME>:
    The name of the OS distribution that the Docker image is based on. Currently, <OS_NAME> must be one of the following values:
        ubuntu16.04         Use Ubuntu 16.04 as the base image
        centos7.2           Use CentOS 7.2 as the base image

The resulting Docker image will have "occlum/occlum:<OCCLUM_LABEL>-<OS_NAME>" as its label.
EOF
    exit 1
}

set -e

if [[ ( "$#" < 2 ) ]] ; then
    report_error
fi

occlum_label=$1
os_name=$2

if [ "$os_name" != "ubuntu16.04" ] && [ "$os_name" != "centos7.2" ];then
    report_error
fi

cd "$script_dir/.."
docker build -f "$script_dir/Dockerfile.$os_name" -t "occlum/occlum:$occlum_label-$os_name" .
