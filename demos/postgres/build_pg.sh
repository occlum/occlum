#!/bin/bash
set -e


function dl_build_install_pg()
{
    rm -rf pg15
    git clone -b REL_15_STABLE --depth=1 https://github.com/postgres/postgres.git pg15

    pushd pg15
    git apply ../0001-Make-pg15-running-on-Occlum.patch
    ./configure --without-readline
    make -j$(nproc)
    make install
    popd
}

function dl_build_install_proj()
{
    # build and install proj
    rm -rf proj-4.9.1*
    curl -O http://download.osgeo.org/proj/proj-4.9.1.tar.gz
    tar zxvf proj-4.9.1.tar.gz
    pushd proj-4.9.1
    ./configure
    make -j$(nproc)
    make install
    popd
}

function dl_build_install_postgis()
{
    # build and install postgis
    rm -rf postgis-3.3.3dev*
    wget http://postgis.net/stuff/postgis-3.3.3dev.tar.gz
    tar -xvzf postgis-3.3.3dev.tar.gz
    pushd postgis-3.3.3dev
    ./configure --with-pgconfig=/usr/local/pgsql/bin/pg_config --with-projdir=/usr/local --without-protobuf
    make -j$(nproc)
    make install
    popd
}

function dl_build_install_citus()
{
    rm -rf citus
    git clone -b v11.2.0 https://github.com/citusdata/citus.git
    pushd citus
    PG_CONFIG=/usr/local/pgsql/bin/pg_config ./configure
    make -j$(nproc)
    make install
    popd
}


echo "Download, build and install Postgresql ..."
dl_build_install_pg

echo "Download, build and install PostGIS ..."
# Please refer to https://postgis.net/docs/postgis_installation.html#make_install_postgis_extensions
dl_build_install_proj
dl_build_install_postgis
dl_build_install_citus
