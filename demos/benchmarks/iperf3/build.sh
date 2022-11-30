#! /bin/bash
set -e

script_dir="$( cd "$( dirname "${BASH_SOURCE[0]}"  )" >/dev/null 2>&1 && pwd )"

export IPERF3_INSTALL_DIR=${script_dir}/iperf-install

function dl_and_build_iperf()
{
    rm -rf iperf-*
    rm -rf 3.11*
    mkdir -p ${IPERF3_INSTALL_DIR}
    wget https://github.com/esnet/iperf/archive/refs/tags/3.11.tar.gz
    tar zxf 3.11.tar.gz
    pushd iperf-3.11
    ./configure
    make install exec_prefix=${IPERF3_INSTALL_DIR}
    popd
}

function build_occlum_instance()
{
    name=$1
    rm -rf ${name}
    occlum new ${name}
    pushd ${name}
    copy_bom -f ../iperf3.yaml --root image --include-dir /opt/occlum/etc/template

    new_json="$(jq '.resource_limits.user_space_size = "1000MB" |
            .resource_limits.max_num_of_threads = 64 ' Occlum.json)" && \
    echo "${new_json}" > Occlum.json

    occlum build
    popd
}

dl_and_build_iperf
build_occlum_instance occlum_server
build_occlum_instance occlum_client
