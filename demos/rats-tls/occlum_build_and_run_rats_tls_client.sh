#!/bin/bash
set -e

rm -rf occlum_client
occlum new occlum_client
cd occlum_client

# Copy libs/files to prepare occlum image
rm -rf image
copy_bom -f ../client.yaml --root image --include-dir /opt/occlum/etc/template

occlum build
occlum run /bin/rats-tls-client -m
