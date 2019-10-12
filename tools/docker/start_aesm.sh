#!/bin/bash
# Start AESM service required by Intel SGX SDK if it is not running
if ! pgrep "aesm_service" > /dev/null ; then
    /opt/intel/sgxpsw/aesm/aesm_service
fi
