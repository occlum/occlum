#! /bin/bash
set -e

rm -rf occlum_instance
occlum new occlum_instance

cd occlum_instance
rm -rf image
copy_bom -f ../sysbench.yaml --root image --include-dir /opt/occlum/etc/template

new_json="$(jq '.resource_limits.user_space_size = "800MB" |
                .resource_limits.max_num_of_threads = 256 ' Occlum.json)" && \
    echo "${new_json}" > Occlum.json

occlum build
#occlum run /bin/sysbench threads --threads=200 --thread-yields=100 --thread-locks=4 --time=10 run
