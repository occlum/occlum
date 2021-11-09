#!/bin/bash
set -e

SCRIPT_DIR=$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )
bomfile=${SCRIPT_DIR}/fish.yaml

rm -rf occlum-instance
occlum new occlum-instance
cd occlum-instance

new_json="$(jq '.resource_limits.user_space_size = "512MB" |
            .resource_limits.kernel_space_heap_size = "64MB" |
            .env.default = [ "OCCLUM=yes", "HOME=/root" ]' Occlum.json)" && \
    echo "${new_json}" > Occlum.json

rm -rf image
copy_bom -f $bomfile --root image --include-dir /opt/occlum/etc/template

occlum build
echo -e "\nBuild done. Running fish script ..."
occlum run /bin/fish_script.sh
