#!/bin/bash
set -e

BLUE='\033[1;34m'
NC='\033[0m'

script_dir="$( cd "$( dirname "${BASH_SOURCE[0]}"  )" >/dev/null 2>&1 && pwd )"
python_dir="$script_dir/occlum_instance/image/opt/python-occlum"


function generate_ca_files()
{
    cn_name=${1:-"localhost"}
    # Generate CA files
    openssl req -x509 -nodes -days 1825 -newkey rsa:2048 -keyout myCA.key -out myCA.pem -subj "/CN=${cn_name}"
    # Prepare test private key
    openssl genrsa -out test.key 2048
    # Use private key to generate a Certificate Sign Request
    openssl req -new -key test.key -out test.csr -subj "/C=CN/ST=Shanghai/L=Shanghai/O=Ant/CN=${cn_name}"
    # Use CA private key and CA file to sign test CSR
    openssl x509 -req -in test.csr -CA myCA.pem -CAkey myCA.key -CAcreateserial -out test.crt -days 825 -sha256
}

function build_instance()
{
    rm -rf occlum_instance* && occlum new occlum_instance
    pushd occlum_instance
    rm -rf image
    copy_bom -f ../pytorch.yaml --root image --include-dir /opt/occlum/etc/template
    rm -rf $script_dir/python-occlum

    if [ ! -d $python_dir ];then
        echo "Error: cannot stat '$python_dir' directory"
        exit 1
    fi

    new_json="$(jq '.resource_limits.user_space_size = "1MB" |
                    .resource_limits.user_space_max_size = "5000MB" |
                    .resource_limits.kernel_space_heap_size = "1MB" |
                    .resource_limits.kernel_space_heap_max_size = "400MB" |
                    .resource_limits.max_num_of_threads = 64 |
                    .env.untrusted += [ "MASTER_ADDR", "MASTER_PORT", "WORLD_SIZE", "RANK", "OMP_NUM_THREADS", "HOME" ] |
                    .env.default += ["GLOO_DEVICE_TRANSPORT=TCP_TLS"] |
                    .env.default += ["GLOO_DEVICE_TRANSPORT_TCP_TLS_PKEY=/ppml/certs/test.key"] |
                    .env.default += ["GLOO_DEVICE_TRANSPORT_TCP_TLS_CERT=/ppml/certs/test.crt"] |
                    .env.default += ["GLOO_DEVICE_TRANSPORT_TCP_TLS_CA_FILE=/ppml/certs/myCA.pem"] |
                    .env.default += ["PYTHONHOME=/opt/python-occlum"] |
                    .env.default += [ "MASTER_ADDR=127.0.0.1", "MASTER_PORT=29500" ] ' Occlum.json)" && \
    echo "${new_json}" > Occlum.json
    occlum build
    # Delete image folder to save disk
    rm -rf image
    popd
}

generate_ca_files
build_instance

# Test instance for 2 nodes distributed pytorch training
cp -r occlum_instance occlum_instance_2
