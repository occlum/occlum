#!/bin/bash
set -e

BLUE='\033[1;34m'
NC='\033[0m'
conf_dir=conf

id=$([ -f "$pid" ] && echo $(wc -l < "$pid") || echo "0")
FLINK_LOG_PREFIX="/host/flink--$postfix-${id}"
log="${FLINK_LOG_PREFIX}.log"
out="./flink--$postfix-${id}.out"

core_num=1
job_manager_host=127.0.0.1
job_manager_rest_port=8081
job_manager_rpc_port=6123

task_manager_host=127.0.0.1
task_manager_data_port=6124
task_manager_rpc_port=6125
task_manager_taskslots_num=1

flink_home=$FLINK_HOME
flink_version=$FLINK_VERSION

run_taskmanager() {
    # enter occlum image
    cd flink

    #if conf_dir exists, use the new configurations.
    if [[ -d $conf_dir && "$(ls -A $conf_dir)" ]]; then
        cp -r $conf_dir/* image/opt/conf/
        occlum build
    fi

    echo -e "${BLUE}occlum run JVM taskmanager${NC}"
    echo -e "${BLUE}logfile=$log${NC}"
    # start task manager in occlum
    occlum run /usr/lib/jvm/java-11-openjdk-amd64/bin/java \
    -XX:+UseG1GC -Xmx1152m -Xms1152m -XX:MaxDirectMemorySize=512m -XX:MaxMetaspaceSize=256m \
    -Dos.name=Linux \
    -XX:ActiveProcessorCount=${core_num} \
    -Dlog.file=$log \
    -Dlog4j.configuration=file:/opt/conf/log4j.properties \
    -Dlogback.configurationFile=file:/opt/conf/logback.xml \
    -classpath /bin/lib/* org.apache.flink.runtime.taskexecutor.TaskManagerRunner \
    -Dorg.apache.flink.shaded.netty4.io.netty.tryReflectionSetAccessible=true \
    -Dorg.apache.flink.shaded.netty4.io.netty.eventLoopThreads=${core_num} \
    -Dcom.intel.analytics.zoo.shaded.io.netty.tryReflectionSetAccessible=true \
    --configDir /opt/conf \
    -D rest.bind-address=${job_manager_host} \
    -D rest.bind-port=${job_manager_rest_port} \
    -D jobmanager.rpc.address=${job_manager_host} \
    -D jobmanager.rpc.port=${job_manager_rpc_port} \
    -D jobmanager.heap.size=5g \
    -D taskmanager.host=${task_manager_host} \
    -D taskmanager.data.port=${task_manager_data_port} \
    -D taskmanager.rpc.port=${task_manager_rpc_port} \
    -D taskmanager.numberOfTaskSlots=${task_manager_taskslots_num} \
    -D taskmanager.cpu.cores=${core_num} \
    -D taskmanager.memory.framework.off-heap.size=256mb \
    -D taskmanager.memory.network.max=256mb \
    -D taskmanager.memory.network.min=256mb \
    -D taskmanager.memory.framework.heap.size=128mb \
    -D taskmanager.memory.managed.size=800mb \
    -D taskmanager.cpu.cores=1.0 \
    -D taskmanager.memory.task.heap.size=1024mb \
    -D taskmanager.memory.task.off-heap.size=0mb 2>&1 | tee $out &
}

run_taskmanager
