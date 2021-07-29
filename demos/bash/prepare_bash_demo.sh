#! /bin/bash
set -e

current=$(pwd)
bash_src="$current/bash_for_occlum"
bash_git="https://github.com/occlum/bash.git"
bash_branch="bash_5.1_for_occlum"
busybox_src="$current/busybox"

if [ ! -d "$bash_src" ]; then
    # Download and configure Bash
    cd $current
    git clone -b $bash_branch $bash_git bash_for_occlum
    cd bash_for_occlum && git checkout $bash_branch
    if [ "$DEBUG" == "1" ]; then
        CFLAGS="-D DEBUG=1 -g -O0" ./configure --enable-debugger
    else
        ./configure
    fi

    # Build
    make -j$(nproc)
    echo "Bash is ready."
fi

if [ ! -d "$busybox_src" ]; then
    cd $current
    busybox_config="$current/../fish/.config"
    git clone -b 1_31_1 --depth 1 https://github.com/mirror/busybox.git
    cd busybox
    # CROSS_COMPILE=/opt/occlum/toolchains/gcc/bin/occlum-
    make defconfig
    cp $busybox_config .
    make -j$(nproc)
fi
