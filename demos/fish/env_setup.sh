#!/bin/bash
set -e

#
rm -rf occlum-context
mkdir occlum-context && cd occlum-context
occlum init
cp ../Occlum.json .
cp ../fish-shell/build/fish image/bin
cp ../busybox/busybox image/bin
cp ../fish_script.sh image

occlum build
