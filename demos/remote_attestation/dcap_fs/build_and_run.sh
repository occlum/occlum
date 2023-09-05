#!/bin/bash
set -e

BLUE='\033[1;34m'
NC='\033[0m'
INSTANCE_DIR="occlum_instance"


echo "*** Build and run dcap fs demo ***"
bomfile="../dcap.yaml"
LIBPATH="/opt/occlum/toolchains/dcap_lib/glibc"
INCPATH="/opt/occlum/toolchains/dcap_lib/inc"

LIBPATH=$LIBPATH make -C c_app clean
LIBPATH=$LIBPATH INCPATH=$INCPATH make -C c_app

rm -rf ${INSTANCE_DIR} && occlum new ${INSTANCE_DIR}
cd ${INSTANCE_DIR}

rm -rf image
copy_bom -f $bomfile --root image --include-dir /opt/occlum/etc/template

occlum build
occlum run /bin/dcap_fs
