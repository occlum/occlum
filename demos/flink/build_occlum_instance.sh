#!/bin/bash
set -e

BLUE='\033[1;34m'
NC='\033[0m'

RPC_BIND_PORT=8089
OCCLUM_USER_SPACE_SIZE=8GB

build_instance() {
    postfix=$1
    rm -rf occlum_instance*
    occlum new occlum_instance_$postfix
    cd occlum_instance_$postfix
    new_json="$(jq '.resource_limits.user_space_size = "1MB" |
        .resource_limits.user_space_max_size = "OCCLUM_USER_SPACE_SIZE" |
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

    # Use hostfs for flink conf in k8s mode
    if [ "$postfix" == "k8s" ]; then
        # Increase user space size for k8s mode
        OCCLUM_USER_SPACE_SIZE=16GB

        rm -rf image/opt/flink*/conf/*
        new_json="$(cat Occlum.json | jq '.mount+=[{"target": "/opt/flink/conf", "type": "hostfs","source": "/opt/flink/conf-copy"}]')" && \
        echo "${new_json}" > Occlum.json

        # use host secrets
        mkdir -p image/var/run/secrets
        new_json="$(cat Occlum.json | jq '.mount+=[{"target": "/var/run/secrets", "type": "hostfs","source": "/var/run/secrets-copy"}]')" && \
        echo "${new_json}" > Occlum.json

        # k8s pod template
        mkdir -p image/opt/flink/pod-template
        new_json="$(cat Occlum.json | jq '.mount+=[{"target": "/opt/flink/pod-template", "type": "hostfs","source": "/opt/flink/pod-template-copy"}]')" && \
        echo "${new_json}" > Occlum.json
    fi

    # Update user size
    sed -i "s/OCCLUM_USER_SPACE_SIZE/$OCCLUM_USER_SPACE_SIZE/g" Occlum.json

    occlum build
    occlum package --debug
    cd ..
}

update_flink_conf() {
    echo "rest.port: $RPC_BIND_PORT" >> flink-1.15.2/conf/flink-conf.yaml
}


if [ "$1" == "k8s" ]; then
    echo "do occlum instance build for k8s mode"
    build_instance k8s
else
    update_flink_conf
    build_instance jobmanager
    # flink job manager and taks manager use the same occlum instance
    cp -rf occlum_instance_jobmanager occlum_instance_taskmanager
fi
