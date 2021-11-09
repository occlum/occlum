#!/bin/bash
set -e

src_dir="./source_code"
vault="$src_dir/bin/vault"

if [ -f "$vault" ]; then
    echo "Warning: the current working directory has Vault already downloaded and built"
    exit 1
fi

# download the source code of Vault v1.7.0
wget https://github.com/hashicorp/vault/archive/refs/tags/v1.7.0.tar.gz
mkdir -p $src_dir && tar -xvzf v1.7.0.tar.gz -C $src_dir --strip-components=1

# build Vault executable
pushd $src_dir
occlum-go build -o bin/vault
popd
