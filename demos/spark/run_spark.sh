#!/bin/bash
set -e

BLUE='\033[1;34m'
NC='\033[0m'

SPARK_HOME=/opt/spark
META_SPACE=256m

init_instance() {
    # init occlum instance
    rm -rf occlum_instance && mkdir occlum_instance
    cd occlum_instance
    occlum init
    new_json="$(jq '.resource_limits.user_space_size = "SGX_MEM_SIZE" |
        .resource_limits.max_num_of_threads = "SGX_THREAD" |
        .process.default_heap_size = "SGX_HEAP" |
        .metadata.debuggable = false |
        .resource_limits.kernel_space_heap_size="SGX_KERNEL_HEAP" |
        .entry_points = [ "/usr/lib/jvm/java-11-openjdk-amd64/bin" ] |
        .env.untrusted = [ "DMLC_TRACKER_URI", "SPARK_DRIVER_URL", "SPARK_TESTING" ] |
        .env.default = [ "LD_LIBRARY_PATH=/usr/lib/jvm/java-11-openjdk-amd64/lib/server:/usr/lib/jvm/java-11-openjdk-amd64/lib:/usr/lib/jvm/java-11-openjdk-amd64/../lib:/lib","SPARK_ENV_LOADED=1","SPARK_SCALA_VERSION=2.12",""]' Occlum.json)" && \
    echo "${new_json}" > Occlum.json

    if [[ -z "$SGX_MEM_SIZE" ]]; then
        sed -i "s/SGX_MEM_SIZE/16GB/g" Occlum.json
    else
        sed -i "s/SGX_MEM_SIZE/${SGX_MEM_SIZE}/g" Occlum.json
    fi

    if [[ -z "$SGX_THREAD" ]]; then
        sed -i "s/\"SGX_THREAD\"/512/g" Occlum.json
    else
        sed -i "s/\"SGX_THREAD\"/${SGX_THREAD}/g" Occlum.json
    fi

    if [[ -z "$SGX_HEAP" ]]; then
        sed -i "s/SGX_HEAP/512MB/g" Occlum.json
    else
        sed -i "s/SGX_HEAP/${SGX_HEAP}/g" Occlum.json
    fi

    if [[ -z "$SGX_KERNEL_HEAP" ]]; then
        sed -i "s/SGX_KERNEL_HEAP/1GB/g" Occlum.json
    else
        sed -i "s/SGX_KERNEL_HEAP/${SGX_KERNEL_HEAP}/g" Occlum.json
    fi

}

build_spark() {
    # Copy spark, jvm, libs into instance
    rm -rf image
    copy_bom -f ../spark.yaml --root image --include-dir /opt/occlum/etc/template
    occlum build -f
}

run_spark_pi() {
    init_instance
    build_spark
    echo -e "${BLUE}occlum run spark Pi${NC}"
    occlum run /usr/lib/jvm/java-11-openjdk-amd64/bin/java \
                -XX:-UseCompressedOops -XX:MaxMetaspaceSize=$META_SPACE \
                -XX:ActiveProcessorCount=4 \
                -Divy.home="/tmp/.ivy" \
                -Dos.name="Linux" \
                -cp "$SPARK_HOME/conf/:$SPARK_HOME/jars/*" \
                -Xmx10g org.apache.spark.deploy.SparkSubmit \
                --jars $SPARK_HOME/examples/jars/spark-examples_2.12-3.1.2.jar,$SPARK_HOME/examples/jars/scopt_2.12-3.7.1.jar \
                --class org.apache.spark.examples.SparkPi spark-internal
}

arg=$1
case "$arg" in
    init)
        init_instance
        build_spark
        ;;
    pi)
        run_spark_pi
        cd ../
        ;;
esac
