#!/bin/bash
set -e

SRC=fio_src

# Download FIO
if [ ! -d $SRC ];then
    rm -rf $SRC && mkdir $SRC
    cd $SRC
    git clone https://github.com/axboe/fio.git .
    git checkout tags/fio-3.28
    git apply ../disable-fadvise.diff
else
    cd $SRC
fi

# Build FIO
./configure --disable-shm --cc=occlum-gcc
make

echo "Build FIO success!"
