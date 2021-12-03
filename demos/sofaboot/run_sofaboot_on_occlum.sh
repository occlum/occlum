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
                .process.default_mmap_size = "1400MB" |
                .entry_points = [ "JDK_BIN" ] |
                .env.default = [ "JDK_LIB_PATH" ]' Occlum.json)" && \
    echo "${new_json}" > Occlum.json

    # Update JDK bin Path
    sed -i "s#JDK_BIN#${JDK_BIN}#g" Occlum.json

    # Update JDK Lib Path
    sed -i "s#JDK_LIB_PATH#${JDK_LIB_PATH}#g" Occlum.json
}

build_sofa() {
    # Copy JVM and JAR file into Occlum instance and build
    rm -rf image
    copy_bom -f ${bomfile} --root image --include-dir /opt/occlum/etc/template
    occlum build
}

run_sofa() {
    jar_path=./sofa-boot-guides/sofaboot-sample-standard/target/boot/sofaboot-sample-standard-web-0.0.1-SNAPSHOT-executable.jar
    check_file_exist ${jar_path}
    jar_file=`basename "${jar_path}"`
    init_instance
    build_sofa
    echo -e "${BLUE}occlum run SOFABoot demo${NC}"
    occlum run ${jdk_path}/jre/bin/java -Xmx512m -XX:-UseCompressedOops -XX:MaxMetaspaceSize=64m -Dos.name=Linux -jar /usr/lib/spring/${jar_file} > ../sofaboot.log &
}

if [[ $1 == "jdk8" ]]; then
    echo ""
    echo "*** Run sofaboot demo with openjdk 8 in Occlum ***"
    bomfile="../sofaboot_jdk8.yaml"
    jdk_path="/usr/lib/jvm/java-1.8-openjdk"
    JDK_LIB_PATH="LD_LIBRARY_PATH=${jdk_path}/jre/lib:${jdk_path}/lib"
else
    echo ""
    echo "*** Run sofaboot demo with openjdk 11 in Occlum ***"
    bomfile="../sofaboot.yaml"
    jdk_path="/usr/lib/jvm/java-11-alibaba-dragonwell"
    JDK_LIB_PATH="LD_LIBRARY_PATH=${jdk_path}/jre/lib/server:${jdk_path}/jre/lib:${jdk_path}/jre/../lib"
fi

JDK_BIN="${jdk_path}/jre/bin"
run_sofa
