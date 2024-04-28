#!/bin/bash
set -e

BLUE='\033[1;34m'
NC='\033[0m'

script_dir="$( cd "$( dirname "${BASH_SOURCE[0]}"  )" >/dev/null 2>&1 && pwd )"
FLINK_BIND_PORT=8089

run_jobmanager() {
    logfile="flink--standalonesession-0.log"
    echo -e "${BLUE}occlum run JVM jobmanager${NC}"
    echo -e "${BLUE}logfile=$logfile${NC}"

    cd occlum_instance_jobmanager
    occlum run /usr/lib/jvm/java-11-openjdk-amd64/bin/java \
        -Dos.name=Linux -XX:ActiveProcessorCount=4 -Xmx800m -Xms800m \
        -XX:MaxMetaspaceSize=256m -Dlog.file=/host/$logfile \
        -Dlog4j.configuration=file:/opt/flink/conf/log4j.properties \
        -Dlog4j.configurationFile=file:/opt/flink/conf/log4j.properties \
        -Dlogback.configurationFile=file:/opt/flink/conf/logback.xml \
        -classpath /opt/flink/lib/* org.apache.flink.runtime.entrypoint.StandaloneSessionClusterEntrypoint \
        -D jobmanager.memory.off-heap.size=128mb \
        -D jobmanager.memory.jvm-overhead.min=192mb \
        -D jobmanager.memory.jvm-metaspace.size=256mb \
        -D jobmanager.memory.jvm-overhead.max=192mb \
        -D rest.bind-port=$FLINK_BIND_PORT \
        -D rest.bind-address=0.0.0.0 \
        --configDir /opt/flink/conf \
        --executionMode cluster \
        &
    cd ..
}

run_taskmanager() {
    logfile="flink--taskmanager-0.log"
    echo -e "${BLUE}occlum run JVM taskmanager${NC}"
    echo -e "${BLUE}logfile=$logfile${NC}"

    cd occlum_instance_taskmanager
    occlum run /usr/lib/jvm/java-11-openjdk-amd64/bin/java \
        -Dos.name=Linux -XX:ActiveProcessorCount=2 -XX:+UseG1GC \
        -Xmx600m -Xms600m -XX:MaxMetaspaceSize=256m \
        -Dlog.file=/host/$logfile \
        -Dlog4j.configuration=file:/opt/flink/conf/log4j.properties \
        -Dlog4j.configurationFile=file:/opt/flink/conf/log4j.properties \
        -Dlogback.configurationFile=file:/opt/flink/conf/logback.xml \
        -classpath /opt/flink/lib/* org.apache.flink.runtime.taskexecutor.TaskManagerRunner \
        --configDir /opt/flink/conf -D taskmanager.memory.network.min=128mb \
        -D taskmanager.cpu.cores=1.0 -D taskmanager.memory.task.off-heap.size=0b \
        -D taskmanager.memory.jvm-metaspace.size=256mb -D external-resources=none \
        -D taskmanager.memory.jvm-overhead.min=192mb \
        -D taskmanager.memory.framework.off-heap.size=128mb \
        -D taskmanager.memory.network.max=128mb \
        -D taskmanager.memory.framework.heap.size=128mb \
        -D taskmanager.memory.managed.size=256mb \
        -D taskmanager.memory.task.heap.size=383mb \
        -D taskmanager.numberOfTaskSlots=1 \
        -D taskmanager.memory.jvm-overhead.max=192mb \
        -D rest.bind-port=$FLINK_BIND_PORT \
        -D rest.bind-address=0.0.0.0 \
        &
    cd ..
}

run_task() {
    cd flink-1.15.2
    ./bin/flink run ./examples/streaming/WordCount.jar
    cd ..
}

arg=$1
case "$arg" in
    jm)
        run_jobmanager
	cd ../
        ;;
    tm)
        run_taskmanager
	cd ../
        ;;
    task)
        run_task
	cd ../
        ;;
esac
