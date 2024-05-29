FROM ubuntu:20.04
LABEL maintainer="Qi Zheng <huaiqing.zq@antgroup.com>"

# Install SGX DCAP and Occlum runtime
ENV APT_KEY_DONT_WARN_ON_DANGEROUS_USAGE=1
ARG PSW_VERSION=2.20.100.4
ARG DCAP_VERSION=1.17.100.4
ARG OCCLUM_VERSION=0.30.1
RUN apt update && DEBIAN_FRONTEND="noninteractive" apt install -y --no-install-recommends gnupg wget ca-certificates jq && \
    echo 'deb [arch=amd64] https://download.01.org/intel-sgx/sgx_repo/ubuntu focal main' | tee /etc/apt/sources.list.d/intel-sgx.list && \
    wget -qO - https://download.01.org/intel-sgx/sgx_repo/ubuntu/intel-sgx-deb.key | apt-key add - && \
    echo 'deb [arch=amd64] https://occlum.io/occlum-package-repos/debian focal main' | tee /etc/apt/sources.list.d/occlum.list && \
    wget -qO - https://occlum.io/occlum-package-repos/debian/public.key | apt-key add - && \
    apt update && apt install -y --no-install-recommends \
        libsgx-launch=$PSW_VERSION-focal1 \
        libsgx-epid=$PSW_VERSION-focal1 \
        libsgx-quote-ex=$PSW_VERSION-focal1 \
        libsgx-urts=$PSW_VERSION-focal1 \
        libsgx-enclave-common=$PSW_VERSION-focal1 \
        libsgx-uae-service=$PSW_VERSION-focal1 \
        libsgx-ae-pce=$PSW_VERSION-focal1 \
        libsgx-ae-qe3=$DCAP_VERSION-focal1 \
        libsgx-ae-id-enclave=$DCAP_VERSION-focal1 \
        libsgx-ae-qve=$DCAP_VERSION-focal1 \
        libsgx-dcap-ql=$DCAP_VERSION-focal1 \
        libsgx-pce-logic=$DCAP_VERSION-focal1 \
        libsgx-qe3-logic=$DCAP_VERSION-focal1 \
        libsgx-dcap-default-qpl=$DCAP_VERSION-focal1 \
        libsgx-dcap-quote-verify=$DCAP_VERSION-focal1 \
        occlum-runtime=$OCCLUM_VERSION-1 \
        gettext openjdk-11-jdk \
        && \
    apt clean && \
    rm -rf /var/lib/apt/lists/*

COPY docker-entrypoint.sh /

RUN mkdir -p /opt/flink
COPY flink-1.15.2 /opt/flink
ADD occlum_instance_k8s/occlum_instance_k8s.tar.gz /opt/flink

ENV FLINK_HOME=/opt/flink
ENV JAVA_HOME=/usr/lib/jvm/java-11-openjdk-amd64
ENV PATH="/opt/occlum/build/bin:/usr/local/occlum/bin:/opt/flink/bin:$PATH"

WORKDIR /opt/flink
ENTRYPOINT ["/docker-entrypoint.sh"]
