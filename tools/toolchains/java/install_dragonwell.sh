#!/bin/bash
THIS_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" >/dev/null 2>&1 && pwd )"
DOWNLOAD_DIR=/tmp/occlum_java_toolchain
INSTALL_DIR=/opt/occlum/toolchains/jvm
JDK=java-11-alibaba-dragonwell

# Exit if any command fails
set -e

# Clean previous download and installation if any
rm -rf ${DOWNLOAD_DIR}
rm -rf ${INSTALL_DIR}/${JDK}

# Create the download directory
mkdir -p ${DOWNLOAD_DIR}
cd ${DOWNLOAD_DIR}

# Download and install Dragonwell JDK
wget https://dragonwell.oss-cn-shanghai.aliyuncs.com/11/11.0.8.3_GA/linux/x64/Alibaba_Dragonwell_11.0.8.3-Enclave-Experimental-WithoutDebugInfo_x64.zip
unzip Alibaba_Dragonwell_11.0.8.3-Enclave-Experimental-WithoutDebugInfo_x64.zip
mkdir -p ${INSTALL_DIR}
mv ${DOWNLOAD_DIR}/jdk ${INSTALL_DIR}/${JDK}
ln -sf . ${INSTALL_DIR}/${JDK}/jre

# Clean the download directory
rm -rf ${DOWNLOAD_DIR}

# Generate the wrappers for executables
mkdir -p ${INSTALL_DIR}/bin
cat > ${INSTALL_DIR}/bin/occlum-java <<EOF
#!/bin/bash
JAVA_HOME="\${JAVA_HOME:-${INSTALL_DIR}/${JDK}}"
LD_LIBRARY_PATH=/opt/occlum/toolchains/gcc/x86_64-linux-musl/lib \${JAVA_HOME}/bin/java "\$@"
EOF

cat > ${INSTALL_DIR}/bin/occlum-javac <<EOF
#!/bin/bash
JAVA_HOME="\${JAVA_HOME:-${INSTALL_DIR}/${JDK}}"
LD_LIBRARY_PATH=/opt/occlum/toolchains/gcc/x86_64-linux-musl/lib \${JAVA_HOME}/bin/javac "\$@"
EOF

chmod +x ${INSTALL_DIR}/bin/occlum-java
chmod +x ${INSTALL_DIR}/bin/occlum-javac
