#!/bin/bash
THIS_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" >/dev/null 2>&1 && pwd )"
DOWNLOAD_DIR=/tmp/occlum_java_toolchain
INSTALL_DIR=/opt/occlum/toolchains/jvm

# Exit if any command fails
set -e

# Clean previous download and installation if any
rm -rf ${DOWNLOAD_DIR}
rm -rf ${INSTALL_DIR}

# Create the download directory
mkdir -p ${DOWNLOAD_DIR}
cd ${DOWNLOAD_DIR}

# Download and install JDK 11
JDK=openjdk-11-for-occlum-0.14.0
wget https://github.com/occlum/occlum/releases/download/0.14.0/${JDK}.tar.gz
tar -xf ${JDK}.tar.gz
mkdir -p ${INSTALL_DIR}/java-11-openjdk
mv ${DOWNLOAD_DIR}/${JDK} ${INSTALL_DIR}/java-11-openjdk/jre

# Clean the download directory
rm -rf ${DOWNLOAD_DIR}

# Generate the wrappers for executables
mkdir -p ${INSTALL_DIR}/bin
cat > ${INSTALL_DIR}/bin/occlum-java <<EOF
#!/bin/bash
LD_LIBRARY_PATH=/opt/occlum/toolchains/gcc/x86_64-linux-musl/lib ${INSTALL_DIR}/java-11-openjdk/jre/bin/java "\$@"
EOF

cat > ${INSTALL_DIR}/bin/occlum-javac <<EOF
#!/bin/bash
LD_LIBRARY_PATH=/opt/occlum/toolchains/gcc/x86_64-linux-musl/lib ${INSTALL_DIR}/java-11-openjdk/jre/bin/javac "\$@"
EOF

chmod +x ${INSTALL_DIR}/bin/occlum-java
chmod +x ${INSTALL_DIR}/bin/occlum-javac
