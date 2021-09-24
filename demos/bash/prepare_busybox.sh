#! /bin/bash
set -e

current=$(pwd)
busybox_src="$current/busybox"
busybox_config="$current/../fish/.config"

rm -rf $busybox_src
git clone -b 1_31_1 --depth 1 https://github.com/mirror/busybox.git

pushd $busybox_src
make clean
make defconfig
cp $busybox_config .

if [[ $1 == "musl" ]]; then
    echo "Building musl-libc version of busybox"
    sed -e 's/.*CONFIG_CROSS_COMPILER_PREFIX.*/CONFIG_CROSS_COMPILER_PREFIX="occlum-"/' -i .config
else
    echo "Building glibc version of busybox"
fi

make -j
popd
