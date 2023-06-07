#!/bin/bash
set -e

SCRIPT_DIR=$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )
bomfile=${SCRIPT_DIR}/fish.yaml

rm -rf occlum-instance
occlum new occlum-instance
cd occlum-instance

yq '.resource_limits.user_space_size.init = "512MB" |
    .resource_limits.kernel_space_heap_size.init = "512MB" |
    .env.default = [ "OCCLUM=yes", "HOME=/root" ]' -i Occlum.yaml

rm -rf image
copy_bom -f $bomfile --root image --include-dir /opt/occlum/etc/template

occlum build
echo -e "\nBuild done. Running fish script ..."
occlum run /bin/fish_script.sh
