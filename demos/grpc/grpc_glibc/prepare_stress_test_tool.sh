#!/bin/bash
set -e

mkdir ghz_src && cd ghz_src
git clone https://github.com/bojand/ghz .
git checkout tags/v0.105.0
make build
