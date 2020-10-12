#!/bin/sh
set -e

workpath=`pwd`

cd ./poi-excel-demo && gradle build customFatJar && cd ../

mkdir -p font-lib/etc && cp -r /etc/fonts ./font-lib/etc
mkdir -p font-lib/lib && cp -d /lib/libuuid.so.1* ./font-lib/lib
mkdir -p font-lib/usr/lib && cp -d /usr/lib/libbz2.so.1* ./font-lib/usr/lib && cp -d /usr/lib/libbrotlicommon.so.1* ./font-lib/usr/lib && cp -d /usr/lib/libbrotlidec.so.1* ./font-lib/usr/lib && \
cp -d /usr/lib/libexpat.so.1* ./font-lib/usr/lib && cp -d /usr/lib/libfontconfig.so.1* ./font-lib/usr/lib && cp -d /usr/lib/libfreetype.so.6* ./font-lib/usr/lib && \
cp -d /usr/lib/libpng16.so.16* ./font-lib/usr/lib
mkdir -p font-lib/usr/share && cp -r /usr/share/fontconfig ./font-lib/usr/share && cp -r /usr/share/fonts ./font-lib/usr/share
