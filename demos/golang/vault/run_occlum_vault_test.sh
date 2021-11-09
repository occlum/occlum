#!/bin/bash
set -e

BLUE='\033[1;34m'
NC='\033[0m'

vault="./source_code/bin/vault"

if [ ! -f $vault ];then
    echo "Error: cannot stat file '$vault'"
    echo "Please see README and build it using Occlum Golang toolchain"
    exit 1
fi

# "127.0.0.1:8200" is the address bound to in "dev" mode
export VAULT_ADDR=http://127.0.0.1:8200
export VAULT_TOKEN=mytoken

echo -e "${BLUE}$vault kv put secret/creds passcode=occlum${NC}"
$vault kv put secret/creds passcode=occlum

echo -e "${BLUE}$vault kv get secret/creds${NC}"
$vault kv get secret/creds
