#!/bin/bash

set -e
script_dir="$( cd "$( dirname "${BASH_SOURCE[0]}"  )" >/dev/null 2>&1 && pwd )"

# Prepare environment
DEPS="git gcc-multilib fuse automake autoconf libtool make gcc libc-dev libssl-dev libc6-dev libgmp-dev libnspr4-dev libnss3-dev pkg-config libfuse-dev libglib2.0-dev expect libtasn1-dev socat tpm-tools python3-twisted gnutls-dev gnutls-bin libjson-glib-dev libseccomp-dev gawk net-tools build-essential devscripts equivs"

apt-get update
apt-get install -y ${DEPS}

# 1. Init occlum workspace
[ -d occlum_instance ] || occlum new occlum_instance

# 2. Install libtpms and swtpm to specified position
[ -d $script_dir/libtpms ] || mkdir $script_dir/libtpms && 
	cd $script_dir/libtpms && 
	git clone -b stable-0.9 https://github.com/stefanberger/libtpms.git . &&
	./autogen.sh --with-openssl --prefix=/usr --with-tpm2 &&
	make -j &&
	make check &&
	make install &&
	cd ..

[ -d $script_dir/swtpm ] || mkdir $script_dir/swtpm && 
	cd $script_dir/swtpm && 
	git clone -b stable-0.9 https://github.com/stefanberger/swtpm.git . &&
    ./autogen.sh --prefix=/usr --with-openssl --with-tss-user=root --with-tss-group=root --without-selinux --without-cuse &&
	make -j &&
	make check &&
	make install
