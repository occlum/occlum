FROM ubuntu:16.04

LABEL maintainer="Tate, Hongliang Tian <tate.thl@antfin.com>"

RUN apt-get update && apt-get install -y --no-install-recommends \
        alien \
        autoconf \
        automake \
        build-essential \
        ca-certificates \
        cmake \
        curl \
        debhelper \
        expect \
        gdb \
        git-core \
        kmod \
        libboost-system-dev \
        libboost-thread-dev \
        libcurl4-openssl-dev \
        libfuse-dev \
        libjsoncpp-dev \
        liblog4cpp5-dev \
        libprotobuf-c0-dev \
        libprotobuf-dev \
        libssl-dev \
        libtool \
        libxml2-dev \
        ocaml \
        pkg-config \
        protobuf-compiler \
        python \
        sudo \
        uuid-dev \
        vim \
        wget \
        && \
    apt-get clean && \
    rm -rf /var/lib/apt/lists/*

# Install SGX SDK
WORKDIR /root/occlum/linux-sgx
RUN git clone -b for_occlum https://github.com/occlum/linux-sgx . && \
    ./download_prebuilt.sh && \
    make && \
    make sdk_install_pkg && \
    make deb_sgx_enclave_common_pkg && \
    dpkg -i ./linux/installer/deb/libsgx-enclave-common/libsgx-enclave-common_*.deb && \
    echo -e 'no\n/opt/intel' | ./linux/installer/bin/sgx_linux_x64_sdk_*.bin && \
    echo 'source /opt/intel/sgxsdk/environment' >> /root/.bashrc && \
    rm -rf /root/occlum/linux-sgx

# Install Rust
ENV OCCLUM_RUST_VERSION=nightly-2019-01-28
RUN curl https://sh.rustup.rs -sSf | \
        sh -s -- --default-toolchain ${OCCLUM_RUST_VERSION} -y && \
    echo 'source /root/.cargo/env' >> /root/.bashrc && \
    rm -rf /root/.cargo/registry && rm -rf /root/.cargo/git

# Install Occlum LLVM
WORKDIR /root/occlum/llvm
RUN git clone -b for_occlum https://github.com/occlum/llvm . && \
    cd /root/occlum/llvm/tools && \
    git clone https://github.com/llvm-mirror/clang && \
        cd clang && \
        git checkout 0513b409d5e && \
    cd /root/occlum/llvm/tools && \
        git clone -b for_occlum https://github.com/occlum/lld && \
    mkdir /root/occlum/llvm-build && cd /root/occlum/llvm-build && \
        cmake -DCMAKE_EXPORT_COMPILE_COMMANDS=True -DLLVM_TARGETS_TO_BUILD="X86" -DCMAKE_INSTALL_PREFIX=/usr/local/occlum/ ../llvm/ && \
        make && \
        make install && \
    rm -rf /root/occlum/llvm && rm -rf /root/occlum/llvm-build
ENV PATH="/usr/local/occlum/bin:$PATH"

# Install Occlum musl libc
WORKDIR /root/occlum/musl
RUN git clone -b for_occlum https://github.com/occlum/musl . && \
    CC=clang ./configure --prefix=/usr/local/occlum --enable-wrapper=clang && \
    make && \
    make install && \
    rm -rf /root/occlum/musl

WORKDIR /root/occlum/
