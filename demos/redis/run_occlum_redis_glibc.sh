#!/bin/bash
set -e

SCRIPT_DIR=$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )
bomfile=${SCRIPT_DIR}/redis_glibc.yaml

# 1. Init Occlum Workspace
rm -rf occlum_instance
occlum new occlum_instance
cd occlum_instance
yq '.resource_limits.user_space_size.max = "320MB"' -i Occlum.yaml

rm -rf image
copy_bom -f $bomfile --root image --include-dir /opt/occlum/etc/template

#occlum build
occlum build
# 3. Run redis server
occlum run /bin/redis-server --save "" --appendonly no &
