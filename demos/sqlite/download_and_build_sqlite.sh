#!/bin/bash
set -e

SQLITE=sqlite-autoconf-3310100
SQLITE_SRC=sqlite_src
DEMO=sqlite_demo

# Download SQLite source files
[ ! -d $SQLITE_SRC ] && rm -f $SQLITE.tar.gz && \
               wget http://www3.sqlite.org/2020/$SQLITE.tar.gz \
               && rm -rf $SQLITE && tar xf $SQLITE.tar.gz \
               && mv $SQLITE $SQLITE_SRC \
               && rm -f $SQLITE.tar.gz

[ -e $DEMO ] && rm -f $DEMO
echo -e "Starting to build $DEMO ..."
occlum-gcc -O2 -I$SQLITE_SRC sqlite_demo.c $SQLITE_SRC/sqlite3.c -lpthread -ldl -o $DEMO
echo -e "Build $DEMO succeed"
