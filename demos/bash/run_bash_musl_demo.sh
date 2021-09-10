#! /bin/bash
set -e

current=$(pwd)
bash_src="$current/bash_for_occlum"
busybox_src="$current/busybox"
occlum_instance="$current/occlum_instance"
# Executable path in Occlum
exec_path=/root/bin

rm -rf occlum_instance
occlum new occlum_instance

cd occlum_instance

cp $bash_src/bash ./image/bin/
cp $busybox_src/busybox image/bin

mkdir -p "$occlum_instance/image/$exec_path"
cp "$current/occlum_bash_test.sh" "$occlum_instance/image/$exec_path"
cp "$current/Occlum.json" "$occlum_instance"

occlum build
occlum run /root/bin/occlum_bash_test.sh
