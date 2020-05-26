#! /bin/fish
busybox echo "Hello-world-from-fish" | busybox awk '$1=$1' FS="-" OFS=" " > /root/output.txt