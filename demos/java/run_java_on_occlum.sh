#!/bin/bash
set -e

BLUE='\033[1;34m'
NC='\033[0m'

show_usage() {
    echo "Error: invalid arguments"
    echo "Usage: $0 web_app/hello/processBuilder/hello_pku"
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
                .entry_points = [ "/usr/lib/jvm/java-11-alibaba-dragonwell/jre/bin" ] |
                .env.default = [ "LD_LIBRARY_PATH=/usr/lib/jvm/java-11-alibaba-dragonwell/jre/lib/server:/usr/lib/jvm/java-11-alibaba-dragonwell/jre/lib:/usr/lib/jvm/java-11-alibaba-dragonwell/jre/../lib" ]' Occlum.json)" && \
    echo "${new_json}" > Occlum.json
}

update_pku_config() {
    new_json="$(jq '.metadata.pkru = 1' Occlum.json)" && echo "${new_json}" > Occlum.json
}

build_web() {
    # Copy JVM and JAR file into Occlum instance and build
    rm -rf image
    copy_bom -f ../webserver.yaml --root image --include-dir /opt/occlum/etc/template
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
    rm -rf image
    copy_bom -f ../hello_world.yaml --root image --include-dir /opt/occlum/etc/template
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

run_hello_pku() {
    hello=./hello_world/Main.class
    check_file_exist ${hello}
    init_instance
    update_pku_config
    build_hello
    echo -e "${BLUE}occlum run JVM hello with PKU enabled${NC}"
    occlum run /usr/lib/jvm/java-11-alibaba-dragonwell/jre/bin/java -Xmx512m -XX:-UseCompressedOops -XX:MaxMetaspaceSize=64m -Dos.name=Linux Main
}

build_processBuilder() {
    # Copy JVM and class file into Occlum instance and build
    rm -rf image
    copy_bom -f ../process_builder.yaml --root image --include-dir /opt/occlum/etc/template
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
    hello_pku)
        run_hello_pku
        ;;
    *)
        show_usage
esac
