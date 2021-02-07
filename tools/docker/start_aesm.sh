#!/bin/bash
# Start AESM service required by Intel SGX SDK if it is not running
if ! pgrep "aesm_service" > /dev/null ; then
    LD_LIBRARY_PATH="/opt/intel/sgx-aesm-service/aesm:$LD_LIBRARY_PATH" /opt/intel/sgx-aesm-service/aesm/aesm_service --no-daemon
fi
