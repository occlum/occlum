#!/bin/bash
set -e

rm -rf occlum-instance
mkdir occlum-instance && cd occlum-instance
occlum init
mkdir -p image/usr/bin
cp ../Occlum.json .
cp ../fish-shell/build/fish image/usr/bin
cp ../busybox/busybox image/usr/bin
cp ../fish_script.sh image/bin
pushd image/bin
ln -s /usr/bin/busybox cat
ln -s /usr/bin/busybox echo
ln -s /usr/bin/busybox awk
popd

occlum build
echo -e "\nBuild done. Running fish script ..."
occlum run /bin/fish_script.sh
