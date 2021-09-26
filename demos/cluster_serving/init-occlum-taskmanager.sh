#!/bin/bash
# set -x

FLINK_VERSION=$FLINK_VERSION
occlum_glibc=/opt/occlum/glibc/lib/
init_instance() {
    # Remove older instance
    rm -rf flink && mkdir flink
    cd flink
    # Init Occlum instance
    occlum init
    new_json="$(jq '.resource_limits.user_space_size = "7000MB" |
                .resource_limits.kernel_space_heap_size="64MB" |
                .resource_limits.max_num_of_threads = 72 |
                .process.default_heap_size = "128MB" |
                .process.default_mmap_size = "6600MB" |
                .entry_points = [ "/usr/lib/jvm/java-11-openjdk-amd64/bin" ] |
                .env.default = [ "LD_LIBRARY_PATH=/usr/lib/jvm/java-11-openjdk-amd64/lib/server:/usr/lib/jvm/java-11-openjdk-amd64/lib:/usr/lib/jvm/java-11-openjdk-amd64/../lib:/lib:/opt/occlum/glibc/lib/", "OMP_NUM_THREADS=1", "KMP_AFFINITY=verbose,granularity=fine,compact,1,0", "KMP_BLOCKTIME=20" ]' Occlum.json)" && \
    echo "${new_json}" > Occlum.json
   }

build_flink() {
    # Copy JVM and class file into Occlum instance and build
    rm -rf image
    copy_bom -f ../cluster_serving.yaml --root image --include-dir /opt/occlum/etc/template
    # build occlum
    occlum build
}

#Build the flink occlum instance
init_instance
build_flink
