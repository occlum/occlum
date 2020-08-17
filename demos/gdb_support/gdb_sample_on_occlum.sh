#!/bin/bash
set -e

rm -rf occlum_instance && mkdir -p occlum_instance
cd occlum_instance
# 1. Initialize a directory as the Occlum instance
occlum init

# 2. Generate a secure Occlum FS image and Occlum SGX enclave
cp ../sample image/bin
occlum build

# 3. Debug the user program inside an SGX enclave with GDB
occlum gdb /bin/sample
