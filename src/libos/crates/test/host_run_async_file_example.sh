#!/bin/bash
source_dir=${PWD}

cd ${source_dir}/../async-file/
cargo run --example seq_read_write --release