#!/bin/bash
set -e

src_dir="./source_code"
nginx="/usr/local/nginx/sbin/nginx"
nginx_version="1.23.3"

if [ -f "$nginx" ]; then
    echo "Warning: the current working directory has NGINX already downloaded and built"
fi

# download the source code of nginx
wget https://nginx.org/download/nginx-"$nginx_version".tar.gz
mkdir -p $src_dir && tar -xzvf nginx-"$nginx_version".tar.gz -C $src_dir --strip-components=1 
rm nginx-"$nginx_version".tar.gz

find $src_dir -type f -exec sed -i 's/fork/vfork/g' {} +
find $src_dir -type f -exec sed -i '/if (setsid()/{N;N;N;s/if (setsid().*\n.*\n.*\n.*/\/\/\n\/\/\n\/\/\n\/\//}' {} +

# build nginx executable
pushd $src_dir
./configure  --with-cc=/opt/occlum/toolchains/gcc/bin/occlum-gcc \
     --with-cpp=/opt/occlum/toolchains/gcc/bin/occlum-g++ \
     --without-http_rewrite_module --with-debug
make && make install
popd
