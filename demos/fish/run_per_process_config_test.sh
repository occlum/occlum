#!/bin/bash
set -e

SCRIPT_DIR=$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )
bomfile=${SCRIPT_DIR}/fish.yaml

option=$1

rm -rf occlum-test
occlum new occlum-test && cd occlum-test

# Set process memory space size to very small values and will fail when running target script using default configuration
yq '.resource_limits.user_space_size.init = "512MB" |
    .resource_limits.kernel_space_heap_size.init= "512MB" |
    .process.default_stack_size = "1MB" |
    .process.default_heap_size = "1MB" |
    .env.default = [ "OCCLUM=yes", "HOME=/root" ]' -i Occlum.yaml

rm -rf image
copy_bom -f $bomfile --root image --include-dir /opt/occlum/etc/template

# If `--without-ulimit` is specified, run without ulimit command and thus will fail
if [[ $1 == "--without-ulimit" ]]; then
    sed -i '/^ulimit -S/ s/^/# &/g' image/bin/test_per_process_config.sh
fi

occlum build
echo -e "\nBuild done. Running fish script ..."
occlum run /bin/test_per_process_config.sh
