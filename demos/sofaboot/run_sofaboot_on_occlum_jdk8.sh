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
    yq '.resource_limits.user_space_size.init = "1680MB" |
        .resource_limits.kernel_space_heap_size.init="512MB" |
        .process.default_heap_size = "256MB" |
        .entry_points = [ "/usr/lib/jvm/java-1.8-openjdk/jre/bin" ] |
        .env.default = [ "LD_LIBRARY_PATH=/usr/lib/jvm/java-1.8-openjdk/jre/lib:/usr/lib/jvm/java-1.8-openjdk/lib" ]' \
        -i Occlum.yaml
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
    occlum run /usr/lib/jvm/java-1.8-openjdk/jre/bin/java -Xmx512m -XX:-UseCompressedOops -XX:MaxMetaspaceSize=64m -Dos.name=Linux -jar /usr/lib/spring/${jar_file} &
}

run_sofa
