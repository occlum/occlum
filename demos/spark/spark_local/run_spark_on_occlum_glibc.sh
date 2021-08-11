#!/bin/bash
set -x
BLUE='\033[1;34m'
NC='\033[0m'
occlum_glibc=/opt/occlum/glibc/lib/

init_instance() {
    # Init Occlum instance
    postfix=$1
    rm -rf occlum_instance_$postfix && mkdir occlum_instance_$postfix
    cd occlum_instance_$postfix
    occlum init
    new_json="$(jq '.resource_limits.user_space_size = "7000MB" |
        .resource_limits.max_num_of_threads = 256 |
        .process.default_heap_size = "128MB" |
        .resource_limits.kernel_space_heap_size="256MB" |
        .process.default_mmap_size = "6500MB" |
        .entry_points = [ "/usr/lib/jvm/java-11-openjdk-amd64/bin" ] |
        .env.default = [ "LD_LIBRARY_PATH=/usr/lib/jvm/java-11-openjdk-amd64/lib/server:/usr/lib/jvm/java-11-openjdk-amd64/lib:/usr/lib/jvm/java-11-openjdk-amd64/../lib:/lib","SPARK_CONF_DIR=/bin/conf","SPARK_ENV_LOADED=1","PYTHONHASHSEED=0","SPARK_HOME=/bin","SPARK_SCALA_VERSION=2.12","SPARK_JARS_DIR=/bin/jars","LAUNCH_CLASSPATH=/bin/jars/*",""]' Occlum.json)" && \
    echo "${new_json}" > Occlum.json

}

build_spark() {
    # Copy JVM and class file into Occlum instance and build
    mkdir -p image/usr/lib/jvm
    cp -r /usr/lib/jvm/java-11-openjdk-amd64 image/usr/lib/jvm
    cp /lib/x86_64-linux-gnu/libz.so.1 image/lib
    cp /lib/x86_64-linux-gnu/libz.so.1 image/$occlum_glibc
    cp $occlum_glibc/libdl.so.2 image/$occlum_glibc
    cp $occlum_glibc/librt.so.1 image/$occlum_glibc
    cp $occlum_glibc/libm.so.6 image/$occlum_glibc
    cp $occlum_glibc/libnss_files.so.2 image/$occlum_glibc
    cp -rf ../spark-3.0.0-bin-hadoop2.7/* image/bin/
    cp -rf ../hosts image/etc/
    cp -rf /etc/ssl image/etc/
    cp -rf /etc/passwd image/etc/
    cp -rf /etc/group image/etc/
    cp -rf /etc/java-11-openjdk image/etc/
    occlum build
}

run_spark_test() {
    init_instance spark
    build_spark
    echo -e "${BLUE}occlum run spark${NC}"
    echo -e "${BLUE}logfile=$log${NC}"
    occlum run /usr/lib/jvm/java-11-openjdk-amd64/bin/java \
		-Xmx1g -XX:-UseCompressedOops -XX:MaxMetaspaceSize=256m \
	        -XX:ActiveProcessorCount=2 \
		-Divy.home="/tmp/.ivy" \
		-Dos.name="Linux" \
    		-cp '/bin/conf/:/bin/jars/*' -Xmx1g org.apache.spark.deploy.SparkSubmit --jars /bin/examples/jars/spark-examples_2.12-3.0.0.jar,/bin/examples/jars/scopt_2.12-3.7.1.jar --class org.apache.spark.examples.SparkPi spark-internal
}


run_spark_test
