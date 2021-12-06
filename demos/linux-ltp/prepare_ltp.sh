#! /bin/bash
set -e

rm -rf ltp_instance
occlum new ltp_instance

cd ltp_instance
rm -rf image
copy_bom -f ../ltp.yaml --root image --include-dir /opt/occlum/etc/template

new_json="$(jq '.resource_limits.user_space_size = "3000MB" |
                .resource_limits.kernel_space_heap_size ="1024MB" |
                .resource_limits.kernel_space_stack_size ="4MB" |
                .resource_limits.max_num_of_threads = 96 |
                .entry_points = [ "/opt/ltp" ] |
                .env.default = [ "OCCLUM=yes", "LTPROOT=/opt/ltp", "TMP=/tmp", "HOME=/root" ]' Occlum.json)" && \
echo "${new_json}" > Occlum.json

occlum build

