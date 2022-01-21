#!/bin/bash
set -ex

get_mr() {
    sgx_sign dump -enclave ../occlum_$1/build/lib/libocclum-libos.signed.so -dumpfile ../metadata_info_$1.txt
    if [ "$2" == "mr_enclave" ]; then
        sed -n -e '/enclave_hash.m/,/metadata->enclave_css.body.isv_prod_id/p' ../metadata_info_$1.txt |head -3|tail -2|xargs|sed 's/0x//g'|sed 's/ //g'
    elif [ "$2" == "mr_signer" ]; then
        tail -2 ../metadata_info_$1.txt |xargs|sed 's/0x//g'|sed 's/ //g'
    fi
}

build_instance() {
    # 1. Init Occlum Workspace
    rm -rf occlum_$postfix
    mkdir occlum_$postfix
    pushd occlum_$postfix
    occlum init
    new_json="$(jq '.resource_limits.user_space_size = "500MB" |
                    .process.default_mmap_size = "256MB"' Occlum.json)" && \
    echo "${new_json}" > Occlum.json

    if [ "$postfix" == "server" ]; then
        bomfile="../grpc_ratls_server.yaml"
    else
        bomfile="../grpc_ratls_client.yaml"
    fi

    rm -rf image
    copy_bom -f $bomfile --root image --include-dir /opt/occlum/etc/template

    if [ "$postfix" == "server" ]; then
        jq ' .verify_mr_enclave = "on" |
             .verify_mr_signer = "on" |
	     .sgx_mrs[0].mr_enclave = ''"'`get_mr client mr_enclave`'" |
	     .sgx_mrs[0].mr_signer = ''"'`get_mr client mr_signer`'" ' ../dynamic_config.json > image/dynamic_config.json 
    
        if [ "$libnss_require" == "y" ]; then
            cp /lib/x86_64-linux-gnu/libnss*.so.2 image/$occlum_glibc
            cp /lib/x86_64-linux-gnu/libresolv.so.2 image/$occlum_glibc
        fi
    fi

    occlum build
    popd
}

if [[ $1 == "musl" ]]; then
    echo "*** Build and musl-libc Occlum instance ***"
else
    echo "*** Build and run glibc Occlum instance ***"
    # glibc version requires libnss
    libnss_require="y"
    occlum_glibc=/opt/occlum/glibc/lib/
fi

postfix=client
build_instance
postfix=server
build_instance

