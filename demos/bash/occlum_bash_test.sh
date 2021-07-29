#! /bin/bash

# Exit when error
set -xe

# Test pipe
busybox echo -e "Hello-world-from-bash\n test" | busybox awk '$1=$1' FS="-" OFS=" " > /root/output.txt
busybox cat /root/output.txt
busybox rm /root/output.txt
busybox ls -l /root/output.txt || true

# Test command substitution
DATE=$(busybox date)
busybox echo $DATE
TEST=$(busybox echo $(busybox date))
busybox echo $TEST

# Test command subsitution and pipe
busybox echo $(busybox echo -e "Hello-world-from-bash\n test" | busybox awk '$1=$1' FS="-" OFS=" ")

# Test multiple redirection
busybox ls . *.blah > log 2>&1 || true
busybox echo "start log:"
busybox cat log
busybox rm log

# Test subshell
SCRIPT_ENV="this is script env"
(
    busybox echo "in subshell:"
    busybox echo $SCRIPT_ENV
    SUBSHELL_ENV="this is subshell env"
    SCRIPT_ENV="this is script env in subshell"
    busybox echo $SUBSHELL_ENV | busybox awk '{print $3}'
    busybox echo $SCRIPT_ENV
)
busybox echo "out subshell:"
busybox echo $SCRIPT_ENV
if [ "$SCRIPT_ENV" != "this is script env" ]; then
    busybox echo "env wrong"
    exit 1
fi

busybox echo $SUBSHELL_ENV
if [ ! -z "$SUBSHELL_ENV" ]; then
    busybox echo "env wrong"
    exit 1
fi

#TEST exec in subshell
(
    exec busybox date
    # This shouldn't be reached
    exit 1
)

# Test unrecognized commands
fake_inst || true

# Test builtin command
cd /opt/occlum/glibc/lib
pwd
cd -

# Test ulimit defined below will overide configuration in Occlum.json
ulimit -Ss 10240 # stack size 10M
ulimit -Sd 40960 # heap size 40M
ulimit -Sv 122880 # virtual memory size 120M (including heap, stack, mmap size)

echo "ulimit result:"
ulimit -a

# Test background process
busybox sleep 1000 &
sleep_pid=$!
kill $sleep_pid

# TODO: Support process substitution

busybox echo "Test is done"
