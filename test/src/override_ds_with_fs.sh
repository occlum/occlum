#!/bin/bash
if [ $# -ne 1 ]; then
    echo "ERROR: the number of given arguments is incorrect!"
    echo
    echo "./use_fs <assembly_file>"
    exit -1
fi

S_FILE=$1
sed -i \
    -e 's/str_size@GOTPCREL/%fs:&/g' \
    -e 's/str_buf@GOTPCREL/%fs:&/g' \
    ${S_FILE}
