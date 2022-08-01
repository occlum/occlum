#! /bin/bash
set -e

script_dir="$( cd "$( dirname "${BASH_SOURCE[0]}"  )" >/dev/null 2>&1 && pwd )"

export IPERF_INSTALL_DIR=${script_dir}/iperf2-install

function dl_and_build_iperf()
{
    rm -rf iperf2-*
    rm -rf ${IPERF_INSTALL_DIR}
    mkdir -p ${IPERF_INSTALL_DIR}
    git clone https://git.code.sf.net/p/iperf2/code iperf2-code
    pushd iperf2-code
    ./configure
    make install exec_prefix=${IPERF_INSTALL_DIR}
    popd
}

function build_occlum_instance()
{
    name=$1
    rm -rf ${name}
    occlum new ${name}
    pushd ${name}
    copy_bom -f ../iperf2.yaml --root image --include-dir /opt/occlum/etc/template

    new_json="$(jq '.resource_limits.user_space_size = "1000MB" |
                .resource_limits.max_num_of_threads = 64 ' Occlum.json)" && \
    echo "${new_json}" > Occlum.json

    occlum build
    popd
}

dl_and_build_iperf
build_occlum_instance occlum_server
build_occlum_instance occlum_client
