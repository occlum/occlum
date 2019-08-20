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

# Install depend cpuid test tool
WORKDIR /tmp
RUN wget http://www.etallen.com/cpuid/cpuid-20180519.x86_64.tar.gz && \
    tar -xf ./cpuid-20180519.x86_64.tar.gz && \
    cp ./cpuid-20180519/cpuid /usr/bin/ && \
    rm -rf /tmp/cpuid-20180519*

# Install SGX SDK
WORKDIR /tmp
RUN git clone https://github.com/occlum/linux-sgx . && \
    ./download_prebuilt.sh && \
    ./compile.sh && \
    ./install.sh && \
    echo 'source /opt/intel/sgxsdk/environment' >> /root/.bashrc && \
    rm -rf /tmp/linux-sgx

# Install Rust
ENV OCCLUM_RUST_VERSION=nightly-2019-01-28
RUN curl https://sh.rustup.rs -sSf | \
        sh -s -- --default-toolchain ${OCCLUM_RUST_VERSION} -y && \
    echo 'source /root/.cargo/env' >> /root/.bashrc && \
    rm -rf /root/.cargo/registry && rm -rf /root/.cargo/git

# Install Occlum toolchain
WORKDIR /tmp
COPY build_toolchain.sh /tmp
RUN ./build_toolchain.sh
ENV PATH="/opt/occlum/build/bin:/usr/local/occlum/bin:$PATH"

WORKDIR /root
