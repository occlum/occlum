#!/bin/bash
set -e

src_dir="./source_code"
vault="$src_dir/bin/vault"
vault_version="1.10.0"

if [ -f "$vault" ]; then
    echo "Warning: the current working directory has Vault already downloaded and built"
    exit 1
fi

# download the source code of Vault
wget https://github.com/hashicorp/vault/archive/refs/tags/v"$vault_version".tar.gz
mkdir -p $src_dir && tar -xvzf v"$vault_version".tar.gz -C $src_dir --strip-components=1

# build Vault executable
pushd $src_dir
occlum-go build -o bin/vault
popd
