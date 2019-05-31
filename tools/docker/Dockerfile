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

# Install Occlum toolchain
WORKDIR /root/occlum/
COPY build_toolchain.sh /root/occlum/
RUN ./build_toolchain.sh
ENV PATH="/usr/local/occlum/bin:$PATH"
