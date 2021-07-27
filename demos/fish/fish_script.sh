#! /usr/bin/fish
command echo "Hello-world-from-fish" | awk '$1=$1' FS="-" OFS=" " > /root/output.txt
cat /root/output.txt

cd /opt/occlum/glibc/lib
/usr/bin/busybox ls -al
pwd
