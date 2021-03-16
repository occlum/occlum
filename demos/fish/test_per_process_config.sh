#! /usr/bin/fish
ulimit -a

# ulimit defined below will overide configuration in Occlum.json
ulimit -Sv 122880 # virtual memory size 120M (including heap, stack, mmap size)
ulimit -Ss 10240 # stack size 10M
ulimit -Sd 40960 # heap size 40M

echo "ulimit result:"
ulimit -a

# A high-memory-consumption process
/usr/bin/busybox dd if=/dev/zero of=/root/test bs=40M count=2
