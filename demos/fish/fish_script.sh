#! /usr/bin/fish
command echo "Hello-world-from-fish" | awk '$1=$1' FS="-" OFS=" " > /root/output.txt
cat /root/output.txt
