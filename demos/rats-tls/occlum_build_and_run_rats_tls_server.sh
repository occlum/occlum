#!/bin/bash
set -e

rm -rf occlum_server
occlum new occlum_server
cd occlum_server

# Copy libs/files to prepare occlum image
rm -rf image
copy_bom -f ../server.yaml --root image --include-dir /opt/occlum/etc/template

occlum build

echo "Run the rats-tls server on background ..."
occlum run /bin/rats-tls-server -m &
