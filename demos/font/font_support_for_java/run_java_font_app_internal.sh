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
    rm -rf occlum_instance && mkdir occlum_instance
    cd occlum_instance
    occlum init
    yq '.resource_limits.user_space_size.init = "1400MB" |
        .resource_limits.kernel_space_heap_size.init="512MB" |
        .process.default_heap_size = "256MB" |
        .entry_points = [ "/usr/lib/jvm/java-11-alibaba-dragonwell/jre/bin" ] |
        .env.default = [ "LD_LIBRARY_PATH=/usr/lib/jvm/java-11-alibaba-dragonwell/jre/lib/server:/usr/lib/jvm/java-11-alibaba-dragonwell/jre/lib:/usr/lib/jvm/java-11-alibaba-dragonwell/jre/../lib" ]' \
        -i Occlum.yaml
}

build_poi_font() {
    # Copy JVM and JAR file into Occlum instance and build
    mkdir -p image/usr/lib/jvm
    cp -r /opt/occlum/toolchains/jvm/java-11-alibaba-dragonwell image/usr/lib/jvm
    cp /usr/local/occlum/x86_64-linux-musl/lib/libz.so.1 image/lib
    cp -r /opt/occlum/font-lib/etc image && cp -r /opt/occlum/font-lib/lib/. image/lib && cp -r /opt/occlum/font-lib/usr/. image/usr
    mkdir -p image/usr/app
    cp ../${jar_path} image/usr/app
    occlum build
}

run_poi_font() {
    jar_path=./poi-excel-demo/build/libs/SXSSFWriteDemoTwo.jar
    check_file_exist ${jar_path}
    jar_file=`basename "${jar_path}"`
    cp -r ./font-lib /opt/occlum
    /opt/occlum/start_aesm.sh
    init_instance
    build_poi_font
    echo -e "${BLUE}occlum run JVM poi font app${NC}"
    occlum run /usr/lib/jvm/java-11-alibaba-dragonwell/jre/bin/java -Xmx512m -XX:-UseCompressedOops -XX:MaxMetaspaceSize=64m -Dos.name=Linux -jar /usr/app/SXSSFWriteDemoTwo.jar
}

run_poi_font
