#! /bin/bash
set -e

yum install -y wget

# Get musl-cross-make config file
CONFIG_SUB_REV=3d5db9ebe860
wget -O $HOME/rpmbuild/SOURCES/config.sub "http://git.savannah.gnu.org/gitweb/?p=config.git;a=blob_plain;f=config.sub;hb=$CONFIG_SUB_REV"

if [ ! -f "$HOME/rpmbuild/SOURCES/musl-$MUSL_VERSION.tar.gz" ]; then
    wget -O $HOME/rpmbuild/SOURCES/musl-$MUSL_VERSION.tar.gz https://github.com/occlum/musl/archive/$MUSL_VERSION.tar.gz
else
    echo "musl-$MUSL_VERSION.tar.gz already exists, skipping download"
fi
