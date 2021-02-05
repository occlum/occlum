#!/bin/bash
source_dir=${PWD}

cd ${source_dir}/../async-file/examples/sgx
make && cd bin
./app