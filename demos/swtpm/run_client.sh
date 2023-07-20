#!/bin/bash

# Download and install TSS
wget https://sourceforge.net/projects/ibmtpm20tss/files/ibmtss1.5.0.tar.gz/download -O ibmtss1.5.0.tar.gz
mkdir ibmtss
cd ibmtss
tar zxvf ../ibmtss1.5.0.tar.gz
cd utils
make -f makefiletpmc


# Set the TPM variables for TSS
export TPM_COMMAND_PORT=2321 TPM_PLATFORM_PORT=2322 TPM_SERVER_NAME=localhost TPM_INTERFACE_TYPE=socsim TPM_SERVER_TYPE=raw


# Start the TPM and test
./startup
./getrandom -by 128
