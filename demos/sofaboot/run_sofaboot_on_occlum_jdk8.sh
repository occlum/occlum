#!/bin/bash
set -e

BLUE='\033[1;34m'
NC='\033[0m'

check_file_exist() {
    file=$1
    if [ ! -f ${file} ];then
        echo "Error: cannot stat file '${file}'"
        echo "Please see README and build it"
        exit 1
    fi
}

init_instance() {
    # Init Occlum instance
    rm -rf occlum_instance && occlum new occlum_instance
    cd occlum_instance
    new_json="$(jq '.resource_limits.user_space_size = "1680MB" |
                .resource_limits.kernel_space_heap_size="64MB" |
                .resource_limits.max_num_of_threads = 64 |
                .process.default_heap_size = "256MB" |
                .entry_points = [ "/usr/lib/jvm/java-8-openjdk-amd64/jre/bin/" ] |
                .env.default = [ "LD_LIBRARY_PATH=/usr/lib/jvm/java-8-openjdk-amd64/jre/lib:/usr/lib/jvm/java-8-openjdk-amd64/lib" ]' Occlum.json)" && \
    echo "${new_json}" > Occlum.json
}

build_sofa() {
    # Copy JVM and JAR file into Occlum instance and build
    rm -rf image
    copy_bom -f ../sofaboot_jdk8.yaml --root image --include-dir /opt/occlum/etc/template
    occlum build
}

run_sofa() {
    jar_path=./sofa-boot-guides/sofaboot-sample-standard/target/boot/sofaboot-sample-standard-web-0.0.1-SNAPSHOT-executable.jar
    check_file_exist ${jar_path}
    jar_file=`basename "${jar_path}"`
    init_instance
    build_sofa
    echo -e "${BLUE}occlum run SOFABoot demo${NC}"
    occlum run /usr/lib/jvm/java-8-openjdk-amd64/jre/bin/java \
        -XX:-UseCompressedOops \
        -XX:ActiveProcessorCount=4 \
        -Dos.name=Linux -jar /usr/lib/spring/${jar_file} &
}

run_sofa
