#!/bin/bash
export https_server=simplest_web_server_ssl
set -e

SCRIPT_DIR=$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )
bomfile=${SCRIPT_DIR}/https.yaml

# 1. Init Occlum Workspace
rm -rf occlum_workspace
mkdir occlum_workspace
cd occlum_workspace
occlum init

# 2. Copy files into Occlum Workspace and Build
rm -rf image
copy_bom -f $bomfile --root image --include-dir /opt/occlum/etc/template

occlum build

# 3. Run https_server
occlum run /bin/$https_server
