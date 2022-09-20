#!/bin/bash
set -e

script_dir="$( cd "$( dirname "${BASH_SOURCE[0]}"  )" >/dev/null 2>&1 && pwd )"

export BASH_DEMO_DIR="${script_dir}/../bash"
export INIT_DIR="${script_dir}/init"

UNIONFS_DIR="${script_dir}/gen_rootfs_instance/mnt_unionfs"
ENCRIP_KEY="c7-32-b3-ed-44-df-ec-7b-25-2d-9a-32-38-8d-58-61"

function build_bash_demo()
{
    pushd ${BASH_DEMO_DIR}
    rm -rf occlum_instance && occlum new occlum_instance

    cd occlum_instance
    rm -rf image
    copy_bom -f ../bash.yaml --root image --include-dir /opt/occlum/etc/template

    new_json="$(jq '.resource_limits.user_space_size = "600MB" |
                    .resource_limits.kernel_space_stack_size ="2MB"	' Occlum.json)" && \
    echo "${new_json}" > Occlum.json

    occlum build
    popd
}

function build_init()
{
    pushd ${INIT_DIR}
    cargo clean
    cargo build --release
    popd
}

function build_and_gen_rootfs()
{
    pushd gen_rootfs
    cargo build
    popd

    # initialize occlum workspace
    rm -rf gen_rootfs_instance && occlum new gen_rootfs_instance
    pushd gen_rootfs_instance

    new_json="$(jq '.resource_limits.user_space_size = "1000MB" |
                .resource_limits.kernel_space_heap_size= "512MB" |
                .resource_limits.kernel_space_stack_size= "16MB" ' Occlum.json)" && \
    echo "${new_json}" > Occlum.json

    rm -rf image
    copy_bom -f ../gen_rootfs.yaml --root image --include-dir /opt/occlum/etc/template

    occlum build

    mkdir -p mnt_unionfs/lower
    mkdir -p mnt_unionfs/upper
    mkdir rootfs
    cp -rf ${BASH_DEMO_DIR}/occlum_instance/image/* rootfs/

    occlum run /bin/gen_rootfs ${ENCRIP_KEY}

    popd
}

function build_boot_template()
{
    rm -rf boot_instance && occlum new boot_instance
    pushd boot_instance

    new_json="$(jq '.resource_limits.user_space_size = "600MB" |
                    .resource_limits.kernel_space_stack_size ="2MB"	' Occlum.json)" && \
    echo "${new_json}" > Occlum.json

    rm -rf image
    copy_bom -f ../boot_template.yaml --root image --include-dir /opt/occlum/etc/template

    # Update init
    rm -rf initfs
    copy_bom -f ../init.yaml --root initfs --include-dir /opt/occlum/etc/template

    occlum build
    popd
}

build_bash_demo
build_and_gen_rootfs
build_init
build_boot_template
