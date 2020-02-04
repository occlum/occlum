#!/bin/bash
set -e

rm -rf occlum_context && mkdir -p occlum_context
cd occlum_context
# 1. Initialize a directory as the Occlum context
occlum init

# 2. Generate a secure Occlum FS image and Occlum SGX enclave
cp ../sample image/bin
occlum build

# 3. Debug the user program inside an SGX enclave with GDB
occlum gdb /bin/sample
