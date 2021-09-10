#! /bin/bash
set -e

current=$(pwd)
bash_src="$current/bash_for_occlum"
bash_git="https://github.com/occlum/bash.git"
bash_branch="bash_5.1_for_occlum"
busybox_src="$current/busybox"
busybox_config="$current/../fish/.config"

if [ ! -d "$bash_src" ]; then
    # Download and configure Bash
    cd $current
    git clone -b $bash_branch $bash_git bash_for_occlum
fi

echo "Building bash ..."
cd bash_for_occlum && git checkout $bash_branch
if [ "$DEBUG" == "1" ]; then
    CFLAGS="-D DEBUG=1 -g -O0" ./configure --enable-debugger
else
    ./configure
fi

make clean
make -j$(nproc)
echo "Bash is ready."

if [ ! -d "$busybox_src" ]; then
    cd $current
    git clone -b 1_31_1 --depth 1 https://github.com/mirror/busybox.git
fi

echo "Building busybox ..."
cd $busybox_src
make defconfig
cp $busybox_config .
make clean
make -j$(nproc)
