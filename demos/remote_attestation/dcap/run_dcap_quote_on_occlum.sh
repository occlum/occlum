#!/bin/bash
set -e

BLUE='\033[1;34m'
NC='\033[0m'
INSTANCE_DIR="occlum_instance"

if [[ $1 == "musl" ]]; then
    echo "*** Build and run musl-libc dcap demo ***"
    bomfile="../dcap-musl.yaml"
    CC=occlum-gcc
    LD=occlum-ld
    LIBPATH="/opt/occlum/toolchains/dcap_lib/musl"
else
    echo "*** Build and run glibc dcap demo ***"
    bomfile="../dcap.yaml"
    CC=gcc
    LD=ld
    LIBPATH="/opt/occlum/toolchains/dcap_lib/glibc"
fi

INCPATH="/opt/occlum/toolchains/dcap_lib/inc"

CC=$CC LD=$LD LIBPATH=$LIBPATH make -C c_app clean
CC=$CC LD=$LD LIBPATH=$LIBPATH INCPATH=$INCPATH make -C c_app

rm -rf ${INSTANCE_DIR} && occlum new ${INSTANCE_DIR}
cd ${INSTANCE_DIR}

rm -rf image
copy_bom -f $bomfile --root image --include-dir /opt/occlum/etc/template

occlum build

echo -e "${BLUE}occlum run rust test /bin/dcap_test${NC}"
occlum run /bin/dcap_test

echo -e "************"

echo -e "${BLUE}occlum run C test /bin/dcap_c_test${NC}"
occlum run /bin/dcap_c_test
