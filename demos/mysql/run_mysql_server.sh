#!/bin/bash
set -e

GREEN='\033[1;32m'
NC='\033[0m'

SCRIPT_DIR=$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )
bomfile=${SCRIPT_DIR}/mysql.yaml

MYSQL=mysql
MYSQLD=mysqld

# 1. Init Occlum instance
rm -rf occlum_instance && occlum new occlum_instance
pushd occlum_instance

yq '.resource_limits.user_space_size.init = "8000MB" |
    .resource_limits.kernel_space_heap_size.init = "2500MB" |
    .resource_limits.kernel_space_heap_size.max = "2500MB" |
    .mount[0].options.layers[1].options.async_sfs_total_size = "35GB" ' -i Occlum.yaml

# 2. Copy files into Occlum instance and build
rm -rf image
copy_bom -f $bomfile --root image --include-dir /opt/occlum/etc/template

occlum build

# 3. Run the program
echo -e "${GREEN}Run mysql server (mysqld) on Occlum${NC}"

occlum start

echo -e "${GREEN}mysql server initialize${NC}"

occlum exec /bin/${MYSQLD} --initialize-insecure --user=root

echo -e "${GREEN}mysql server start${NC}"

occlum exec /bin/${MYSQLD} --user=root

popd
