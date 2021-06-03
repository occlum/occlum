#!/bin/bash
set -e

BLUE='\033[1;34m'
NC='\033[0m'

show_usage() {
    echo "Error: invalid arguments"
    echo "Usage: $0 web_app/hello/processBuilder"
    exit 1
}

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
    new_json="$(jq '.resource_limits.user_space_size = "1680MB" |
                .resource_limits.kernel_space_heap_size="64MB" |
                .resource_limits.max_num_of_threads = 64 |
                .process.default_heap_size = "256MB" |
                .process.default_mmap_size = "1400MB" |
                .entry_points = [ "/usr/lib/jvm/java-11-alibaba-dragonwell/jre/bin" ] |
                .env.default = [ "LD_LIBRARY_PATH=/usr/lib/jvm/java-11-alibaba-dragonwell/jre/lib/server:/usr/lib/jvm/java-11-alibaba-dragonwell/jre/lib:/usr/lib/jvm/java-11-alibaba-dragonwell/jre/../lib" ]' Occlum.json)" && \
    echo "${new_json}" > Occlum.json
}

build_web() {
    # Copy JVM and JAR file into Occlum instance and build
    mkdir -p image/usr/lib/jvm
    cp -r /opt/occlum/toolchains/jvm/java-11-alibaba-dragonwell image/usr/lib/jvm
    cp /usr/local/occlum/x86_64-linux-musl/lib/libz.so.1 image/lib
    mkdir -p image/usr/lib/spring
    cp ../${jar_path} image/usr/lib/spring/
    occlum build
}

run_web() {
    jar_path=./gs-messaging-stomp-websocket/complete/target/gs-messaging-stomp-websocket-0.1.0.jar
    check_file_exist ${jar_path}
    jar_file=`basename "${jar_path}"`
    init_instance
    build_web
    echo -e "${BLUE}occlum run JVM web app${NC}"
    occlum run /usr/lib/jvm/java-11-alibaba-dragonwell/jre/bin/java -Xmx512m -XX:-UseCompressedOops -XX:MaxMetaspaceSize=64m -Dos.name=Linux -jar /usr/lib/spring/${jar_file}
}

build_hello() {
    # Copy JVM and class file into Occlum instance and build
    mkdir -p image/usr/lib/jvm
    cp -r /opt/occlum/toolchains/jvm/java-11-alibaba-dragonwell image/usr/lib/jvm
    cp /usr/local/occlum/x86_64-linux-musl/lib/libz.so.1 image/lib
    cp ../${hello} image
    occlum build
}

run_hello() {
    hello=./hello_world/Main.class
    check_file_exist ${hello}
    init_instance
    build_hello
    echo -e "${BLUE}occlum run JVM hello${NC}"
    occlum run /usr/lib/jvm/java-11-alibaba-dragonwell/jre/bin/java -Xmx512m -XX:-UseCompressedOops -XX:MaxMetaspaceSize=64m -Dos.name=Linux Main
}

build_processBuilder() {
    # Copy JVM and class file into Occlum instance and build
    mkdir -p image/usr/lib/jvm
    cp -r /opt/occlum/toolchains/jvm/java-11-alibaba-dragonwell image/usr/lib/jvm
    cp /usr/local/occlum/x86_64-linux-musl/lib/libz.so.1 image/lib
    cp ../${app} image
    cp /bin/date image/bin/
    # Need bigger user space size for multiprocess
    new_json="$(jq '.resource_limits.user_space_size = "6000MB"' Occlum.json)" && \
    echo "${new_json}" > Occlum.json
    occlum build
}

run_processBuilder() {
    app=./processBuilder/processBuilder.class
    check_file_exist ${app}
    init_instance
    build_processBuilder
    echo -e "${BLUE}occlum run JVM processBuilder${NC}"
    occlum run /usr/lib/jvm/java-11-alibaba-dragonwell/jre/bin/java -Xmx512m -XX:-UseCompressedOops -XX:MaxMetaspaceSize=64m -Dos.name=Linux \
        -Djdk.lang.Process.launchMechanism=posix_spawn processBuilder
}

arg=$1
case "$arg" in
    web_app)
        run_web
        ;;
    hello)
        run_hello
        ;;
    processBuilder)
        run_processBuilder
        ;;
    *)
        show_usage
esac
