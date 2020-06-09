#!/bin/bash
set -e

rm -rf occlum-context
mkdir occlum-context && cd occlum-context
occlum init
cp ../Occlum.json .
cp ../fish-shell/build/fish image/bin
cp ../busybox/busybox image/bin
cp ../fish_script.sh image
pushd image/bin
ln -s /bin/busybox cat
ln -s /bin/busybox echo
ln -s /bin/busybox awk
popd

occlum build
echo -e "\nBuild done. Running fish script ..."
occlum run /bin/fish /fish_script.sh
