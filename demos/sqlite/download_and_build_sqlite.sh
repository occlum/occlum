#!/bin/bash
set -e

SQLITE=sqlite-autoconf-3330000
SQLITE_SRC=sqlite_src
DEMO=sqlite_demo
SPEEDTEST=speedtest1

# Download SQLite source files
[ ! -d $SQLITE_SRC ] && rm -f $SQLITE.tar.gz && \
               wget http://www.sqlite.org/2020/$SQLITE.tar.gz \
               && rm -rf $SQLITE && tar xf $SQLITE.tar.gz \
               && mv $SQLITE $SQLITE_SRC \
               && rm -f $SQLITE.tar.gz
[ -e $DEMO ] && rm -f $DEMO
echo -e "Starting to build $DEMO ..."
occlum-gcc -O2 -I$SQLITE_SRC sqlite_demo.c $SQLITE_SRC/sqlite3.c -lpthread -ldl -o $DEMO
echo -e "Build $DEMO succeed"

[ -e $SPEEDTEST ] && rm -f $SPEEDTEST && rm -f $SPEEDTEST.c
echo -e "Starting to build $SPEEDTEST ..."
wget https://raw.githubusercontent.com/sqlite/sqlite/version-3.33.0/test/$SPEEDTEST.c
occlum-gcc -O6 -I$SQLITE_SRC -DNDEBUG=1 -DSQLITE_ENABLE_MEMSYS5 -DSQLITE_THREADSAFE=2 -DSQLITE_DEFAULT_WORKER_THREADS=32 $SPEEDTEST.c $SQLITE_SRC/sqlite3.c -lpthread -ldl -o $SPEEDTEST
echo -e "Build $SPEEDTEST succeed"

