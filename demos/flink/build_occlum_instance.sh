#!/bin/bash
set -e

BLUE='\033[1;34m'
NC='\033[0m'

RPC_BIND_PORT=8089

build_instance() {
    postfix=$1
    rm -rf occlum_instance*
    occlum new occlum_instance_$postfix
    cd occlum_instance_$postfix
    new_json="$(jq '.resource_limits.user_space_size = "1MB" |
        .resource_limits.user_space_max_size = "8GB" |
        .resource_limits.kernel_space_heap_size="1MB" |
        .resource_limits.kernel_space_heap_max_size="256MB" |
        .resource_limits.max_num_of_threads = 256 |
        .entry_points = [ "/usr/lib/jvm/java-11-openjdk-amd64/bin" ] |
        .env.default = [ "LD_LIBRARY_PATH=/usr/lib/jvm/java-11-openjdk-amd64/lib/server:/usr/lib/jvm/java-11-openjdk-amd64/lib" ] |
        .env.default = [ "FLINK_HOME=/opt/flink" ] |
        .env.default = [ "JAVA_HOME=/usr/lib/jvm/java-11-openjdk-amd64" ] |
        .env.default = [ "HOME=/root" ] |
        .env.untrusted += [ "TZ", "FLINK_CONF_DIR" ]' Occlum.json)" && \
    echo "${new_json}" > Occlum.json

    # Copy JVM and class file into Occlum instance and build
    rm -rf image
    copy_bom -f ../flink.yaml --root image --include-dir /opt/occlum/etc/template
    occlum build
    cd ..
}

update_flink_conf() {
    echo "rest.port: $RPC_BIND_PORT" >> flink-1.15.2/conf/flink-conf.yaml
}

update_flink_conf
build_instance jobmanager
# flink job manager and taks manager use the same occlum instance
cp -rf occlum_instance_jobmanager occlum_instance_taskmanager
