#!/bin/bash
source_dir=${PWD}

cd ${source_dir}/../async-file/
cargo run --example read_write_sample --release