#!/bin/bash

# Update PCCS_URL
line=$(grep -n "pccs_url" /etc/sgx_default_qcnl.conf | cut -d ":" -f 1)
sed -i "${line}c \"pccs_url\": \"${PCCS_URL}\"," /etc/sgx_default_qcnl.conf

exec "$@"
