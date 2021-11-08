#! /bin/bash
set -e
THIS_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}"  )" >/dev/null 2>&1 && pwd )"
INSTALL_DIR=/opt/occlum/toolchains/busybox

busybox_source="https://github.com/mirror/busybox.git"
busybox_tag="1_32_1"
busybox_config=${THIS_DIR}/config

function build_and_install_busybox()
{
    pushd busybox
    git clean -dxf
    git reset HEAD --hard
    make clean
    make defconfig
    cp $busybox_config .config

    if [[ $1 == "musl" ]]; then
        echo "Building musl-libc version of busybox"
        sed -e 's/.*CONFIG_CROSS_COMPILER_PREFIX.*/CONFIG_CROSS_COMPILER_PREFIX="occlum-"/' -i .config
        make -j
        mkdir -p ${INSTALL_DIR}/musl
        cp busybox ${INSTALL_DIR}/musl/
    else
        echo "Building glibc version of busybox"
        make -j
        mkdir -p ${INSTALL_DIR}/glibc
        cp busybox ${INSTALL_DIR}/glibc/
    fi

    popd
}

rm -rf ${INSTALL_DIR}
rm -rf busybox
# Download bash source
git clone -b ${busybox_tag} ${busybox_source}

echo "Building busybox with musl-gcc (occlum-gcc) ..."
build_and_install_busybox musl

echo "Building busybox with gcc ..."
build_and_install_busybox

