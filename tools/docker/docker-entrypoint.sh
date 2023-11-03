#!/bin/bash

# Update PCCS_URL
line=$(grep -n '"pccs_url"' /etc/sgx_default_qcnl.conf | cut -d ":" -f 1)
sed -i "${line}c \"pccs_url\": \"${PCCS_URL}\"," /etc/sgx_default_qcnl.conf
# Update use_secure_cert
line=$(grep -n '"use_secure_cert"' /etc/sgx_default_qcnl.conf | cut -d ":" -f 1)
sed -i "${line}c \"use_secure_cert\": ${USE_SECURE_CERT}" /etc/sgx_default_qcnl.conf

exec "$@"
