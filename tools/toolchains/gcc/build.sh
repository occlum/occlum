#!/bin/bash
THIS_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}"  )" >/dev/null 2>&1 && pwd )"
BUILD_DIR=/tmp/occlum_gcc_toolchain
INSTALL_DIR=/opt/occlum/toolchains/gcc

# Exit if any command fails
set -e

# Clean previous build and installation if any
rm -rf ${BUILD_DIR}
rm -rf ${INSTALL_DIR}

# Create the build directory
mkdir -p ${BUILD_DIR}
cd ${BUILD_DIR}

# Download musl-cross-make project
git clone https://github.com/occlum/musl-cross-make
cd musl-cross-make
git checkout 0.9.9.hotfix

# Let musl-cross-make build for x86-64 Linux
TARGET=x86_64-linux-musl
# We will check out the branch ${MUSL_VER} from ${MUSL_REPO}
MUSL_REPO=https://github.com/occlum/musl
MUSL_VER=1.1.24
# We will use this version of GCC
GCC_VER=8.3.0

# This patch replaces syscall instruction with libc's syscall wrapper
cp ${THIS_DIR}/0014-libgomp-*.diff patches/gcc-${GCC_VER}/

# Build musl-gcc toolchain for Occlum
cat > config.mak <<EOF
TARGET = ${TARGET}
OUTPUT = ${INSTALL_DIR}
COMMON_CONFIG += CFLAGS="-fPIC" CXXFLAGS="-fPIC" LDFLAGS="-pie"

GCC_VER = ${GCC_VER}

MUSL_VER = git-${MUSL_VER}
MUSL_REPO = ${MUSL_REPO}
EOF
make -j$(nproc)
make install

# Remove all source code and build files
rm -rf ${BUILD_DIR}

# Generate the wrappers for executables
cat > ${INSTALL_DIR}/bin/occlum-gcc <<EOF
#!/bin/bash
${INSTALL_DIR}/bin/${TARGET}-gcc -fPIC -pie -Wl,-rpath,${INSTALL_DIR}/${TARGET}/lib "\$@"
EOF

cat > ${INSTALL_DIR}/bin/occlum-g++ <<EOF
#!/bin/bash
${INSTALL_DIR}/bin/${TARGET}-g++ -fPIC -pie -Wl,-rpath,${INSTALL_DIR}/${TARGET}/lib "\$@"
EOF

cat > ${INSTALL_DIR}/bin/occlum-ld <<EOF
#!/bin/bash
${INSTALL_DIR}/bin/${TARGET}-ld -pie -rpath ${INSTALL_DIR}/${TARGET}/lib "\$@"
EOF

chmod +x ${INSTALL_DIR}/bin/occlum-gcc
chmod +x ${INSTALL_DIR}/bin/occlum-g++
chmod +x ${INSTALL_DIR}/bin/occlum-ld

# Make symbolic links
ln -sf ${INSTALL_DIR}/${TARGET}/lib/libc.so /lib/ld-musl-x86_64.so.1
ln -sf ${INSTALL_DIR} /usr/local/occlum
ln -sf ${INSTALL_DIR}/bin/x86_64-linux-musl-gcc-ar ${INSTALL_DIR}/bin/occlum-ar
ln -sf ${INSTALL_DIR}/bin/x86_64-linux-musl-strip ${INSTALL_DIR}/bin/occlum-strip
