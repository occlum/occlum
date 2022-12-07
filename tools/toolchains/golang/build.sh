#!/bin/bash
THIS_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}"  )" >/dev/null 2>&1 && pwd )"
BUILD_DIR=/tmp/occlum_golang_toolchain
INSTALL_DIR=/opt/occlum/toolchains/golang

#go1.18.4_for_occlum or go1.16.3_for_occlum
GO_BRANCH=${1:-"go1.18.4_for_occlum"}

# Exit if any command fails
set -e

# Clean previous build and installation if any
rm -rf ${BUILD_DIR}
rm -rf ${INSTALL_DIR}

# Create the build directory
mkdir -p ${BUILD_DIR}
cd ${BUILD_DIR}

# Download Golang
git clone -b ${GO_BRANCH} https://github.com/occlum/go.git .

# Build Golang
cd src
./make.bash
mv ${BUILD_DIR} ${INSTALL_DIR}

# Generate the wrappers for Go
cat > ${INSTALL_DIR}/bin/occlum-go <<EOF
#!/bin/bash
OCCLUM_GCC="\${CC:-\$(which occlum-gcc)}"
OCCLUM_GOFLAGS="-buildmode=pie \$GOFLAGS"
CC=\$OCCLUM_GCC GOFLAGS=\$OCCLUM_GOFLAGS ${INSTALL_DIR}/bin/go "\$@"
EOF

chmod +x ${INSTALL_DIR}/bin/occlum-go
