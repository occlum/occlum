#!/bin/bash
TESTS="empty hello_world getpid malloc mmap spawn exit_group pthread"

GREEN='\033[1;32m'
NO_COLOR='\033[0m'

set -e

make clean
make TESTS="$TESTS" TEST_DEPS="" BENCHES=""
#export OCCLUM_LOG_LEVEL=debug
export RUST_BACKTRACE=1
cd ../build/test/
for t in $TESTS
do
    /bin/echo -e "[TEST] ${GREEN}$t${NO_COLOR}"
    occlum run /bin/$t
    /bin/echo -e ""
done
