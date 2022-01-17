#!/bin/bash
occlum_glibc=/opt/occlum/glibc/lib/
set -ex

get_mr() {
    sgx_sign dump -enclave ../occlum_instance_$1/build/lib/libocclum-libos.signed.so -dumpfile ../metadata_info_$1.txt
    if [ "$2" == "mr_enclave" ]; then
        sed -n -e '/enclave_hash.m/,/metadata->enclave_css.body.isv_prod_id/p' ../metadata_info_$1.txt |head -3|tail -2|xargs|sed 's/0x//g'|sed 's/ //g'
    elif [ "$2" == "mr_signer" ]; then
        tail -2 ../metadata_info_$1.txt |xargs|sed 's/0x//g'|sed 's/ //g'
    fi
}

build_instance() {
    # 1. Init Occlum Workspace
    rm -rf occlum_instance_$postfix
    mkdir occlum_instance_$postfix
    pushd occlum_instance_$postfix
    occlum init
    new_json="$(jq '.resource_limits.user_space_size = "320MB" |
                    .process.default_mmap_size = "256MB"' Occlum.json)" && \
    echo "${new_json}" > Occlum.json
    # 2. Copy files into Occlum Workspace and Build
    #cp ../dynamic_config.json image/etc/dynamic_config.json
    cp ../dynamic_config.json image/dynamic_config.json
    #cp ../dynamic_config_$postfix.json image/dynamic_config.json
    if [ "$postfix" == "server" ]; then
        jq ' .verify_mr_enclave = "on" |
             .verify_mr_signer = "on" |
	     .sgx_mrs[0].mr_enclave = ''"'`get_mr client mr_enclave`'" |
	     .sgx_mrs[0].mr_signer = ''"'`get_mr client mr_signer`'" ' ../dynamic_config.json > image/dynamic_config.json 
    fi
    mkdir -p image/usr/share/grpc
    cp -rf /share/grpc/*  image/usr/share/grpc/
    cp $occlum_glibc/libdl.so.2 image/$occlum_glibc
    cp $occlum_glibc/librt.so.1 image/$occlum_glibc
    cp $occlum_glibc/libm.so.6 image/$occlum_glibc
    cp /lib/x86_64-linux-gnu/libtinfo.so.5 image/$occlum_glibc
    cp /lib/x86_64-linux-gnu/libnss*.so.2 image/$occlum_glibc
    cp /lib/x86_64-linux-gnu/libresolv.so.2 image/$occlum_glibc
    cp -rf /etc/hostname image/etc/
    cp -rf /etc/ssl image/etc/
    cp -rf /etc/passwd image/etc/
    cp -rf /etc/group image/etc/
    cp -rf /etc/nsswitch.conf image/etc/
    cp -rf /grpc/examples/cpp/ratls/build/* image/bin/
    occlum build
    popd
}

postfix=client
build_instance
postfix=server
build_instance

