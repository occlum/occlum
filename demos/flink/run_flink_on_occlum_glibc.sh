#!/bin/bash
set -e

BLUE='\033[1;34m'
NC='\033[0m'
occlum_glibc=/opt/occlum/glibc/lib/

init_instance() {
    # Init Occlum instance
    postfix=$1
    FLINK_LOG_PREFIX="/host/flink--$postfix-${id}"
    log="${FLINK_LOG_PREFIX}.log"
    out="./flink--$postfix-${id}.out"

    rm -rf occlum_instance_$postfix && mkdir occlum_instance_$postfix
    cd occlum_instance_$postfix
    occlum init
    yq '.resource_limits.user_space_size.init = "5500MB" |
        .process.default_heap_size = "128MB" |
        .resource_limits.kernel_space_heap_size.init="512MB" |
        .entry_points = [ "/usr/lib/jvm/java-11-openjdk-amd64/bin" ] |
        .mount += [{"target": "/host", "type": "hostfs", "source": "."}] |
        .env.default = [ "LD_LIBRARY_PATH=/usr/lib/jvm/java-11-openjdk-amd64/lib/server:/usr/lib/jvm/java-11-openjdk-amd64/lib:/usr/lib/jvm/java-11-openjdk-amd64/../lib:/lib" ]' \
        -i Occlum.yaml
}

build_flink() {
    # Copy JVM and class file into Occlum instance and build
    rm -rf image
    copy_bom -f ../flink.yaml --root image --include-dir /opt/occlum/etc/template
    occlum build
}

run_taskmanager() {
    init_instance taskmanager
    build_flink
    echo -e "${BLUE}occlum run JVM taskmanager${NC}"
    echo -e "${BLUE}logfile=$log${NC}"
    occlum run /usr/lib/jvm/java-11-openjdk-amd64/bin/java \
	-Xmx800m -XX:-UseCompressedOops -XX:MaxMetaspaceSize=256m \
	-XX:ActiveProcessorCount=2 \
	-Dlog.file=$log \
	-Dos.name=Linux \
	-Dlog4j.configuration=file:/bin/conf/log4j.properties \
	-Dlogback.configurationFile=file:/bin/conf/logback.xml \
	-classpath /bin/lib/flink-table-blink_2.11-1.10.1.jar:/bin/lib/flink-table_2.11-1.10.1.jar:/bin/lib/log4j-1.2.17.jar:/bin/lib/slf4j-log4j12-1.7.15.jar:/bin/lib/flink-dist_2.11-1.10.1.jar org.apache.flink.runtime.taskexecutor.TaskManagerRunner \
	--configDir /bin/conf \
	-D taskmanager.memory.network.max=64mb \
	-D taskmanager.memory.network.min=64mb \
	-D taskmanager.memory.managed.size=128mb \
	-D taskmanager.cpu.cores=1.0 \
	-D taskmanager.memory.task.heap.size=256mb \
    &
}

run_task() {
    
    export FLINK_CONF_DIR=$PWD/flink-1.10.1/conf && \
        ./flink-1.10.1/bin/flink run ./flink-1.10.1/examples/streaming/WordCount.jar
}

id=$([ -f "$pid" ] && echo $(wc -l < "$pid") || echo "0")

arg=$1
case "$arg" in
    tm)
        run_taskmanager
	cd ../
        ;;
    task)
        run_task
	cd ../
        ;;
esac
