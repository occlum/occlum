#!/bin/bash

# Update PCCS_URL
line=$(grep -n "PCCS_URL" /etc/sgx_default_qcnl.conf | cut -d ":" -f 1)
sed -i "${line}c PCCS_URL=${PCCS_URL}" /etc/sgx_default_qcnl.conf

exec "$@"
