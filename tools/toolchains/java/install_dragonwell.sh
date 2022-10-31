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
wget https://github.com/alibaba/dragonwell11/releases/download/dragonwell-standard-11.0.16.12_jdk-11.0.16-ga/Alibaba_Dragonwell_Standard_11.0.16.12.8_x64_alpine-linux.tar.gz
tar zxf Alibaba_Dragonwell_Standard_11.0.16.12.8_x64_alpine-linux.tar.gz
mkdir -p ${INSTALL_DIR}
mv ${DOWNLOAD_DIR}/dragonwell-11.0.16.12+8-GA ${INSTALL_DIR}/${JDK}
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
