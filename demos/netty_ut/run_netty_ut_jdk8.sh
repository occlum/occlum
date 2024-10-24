#!/bin/bash
set -e

BLUE='\033[1;34m'
NC='\033[0m'

check_dir_exist() {
    dir=$1
    if [ ! -d ${dir} ];then
        echo "Error: cannot stat file '${dir}'"
        echo "Please see README and build it"
        exit 1
    fi
}

init_instance() {
    # Init Occlum instance
    rm -rf occlum_netty_ut_instance && occlum new occlum_netty_ut_instance
    cd occlum_netty_ut_instance
    new_json="$(jq '.resource_limits.user_space_size = "1MB" |
                .resource_limits.user_space_max_size = "4680MB" |
                .resource_limits.kernel_space_heap_size="1MB" |
                .resource_limits.kernel_space_heap_max_size="64MB" |
                .resource_limits.max_num_of_threads = 128 |
                .process.default_heap_size = "512MB" |
                .entry_points = [ "/usr/lib/jvm/java-8-openjdk-amd64/bin" ] |
                .env.default = [ "LD_LIBRARY_PATH=/usr/lib/jvm/java-8-openjdk-amd64/lib/server:/usr/lib/jvm/java-8-openjdk-amd64/lib:/usr/lib/jvm/java-8-openjdk-amd64/../lib:/lib" ]' Occlum.json)" && \
    echo "${new_json}" > Occlum.json
}

build_netty_ut() {
    # Copy JVM and JAR file into Occlum instance and build
    rm -rf image
    copy_bom -f ../netty-ut-jdk8.yaml --root image --include-dir /opt/occlum/etc/template
    occlum build
}

run_netty_ut() {
    jar_dir_path=./netty
    check_dir_exist ${jar_dir_path}

    init_instance
    build_netty_ut
    echo -e "${BLUE}occlum run netty ut${NC}"
    occlum run /usr/lib/jvm/java-8-openjdk-amd64/bin/java \
        -Xmx1048m -XX:-UseCompressedOops -XX:MaxMetaspaceSize=128m \
        -XX:ActiveProcessorCount=2 \
        -Dreactor.netty.pool.maxIdleTime=60000 \
	-Dos.name=Linux \
        -jar /usr/lib/netty/junit-platform-console-standalone-1.8.2.jar \
        -cp /usr/lib/netty/netty-testsuite-4.1.51.Final.jar:/usr/lib/netty/netty-all-4.1.51.Final.jar:/usr/lib/netty/xz-1.5.jar:/usr/lib/netty/hamcrest-library-1.3.jar:/usr/lib/netty/logback-classic-1.1.7.jar \
        --scan-class-path > netty-test-heap512m.log || true
    cat netty-test-heap512m.log
}       

run_netty_ut
