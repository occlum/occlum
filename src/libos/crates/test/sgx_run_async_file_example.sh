#!/bin/bash
source_dir=${PWD}

cd ${source_dir}/../async-file/examples/sgx/read_write_sample
make && cd bin
./app