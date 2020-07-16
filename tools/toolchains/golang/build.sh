#!/bin/bash
THIS_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}"  )" >/dev/null 2>&1 && pwd )"
BUILD_DIR=/tmp/occlum_golang_toolchain
INSTALL_DIR=/opt/occlum/toolchains/golang

# Exit if any command fails
set -e

# Clean previous build and installation if any
rm -rf ${BUILD_DIR}
rm -rf ${INSTALL_DIR}

# Create the build directory
mkdir -p ${BUILD_DIR}
cd ${BUILD_DIR}

# Download Golang
git clone https://github.com/golang/go .
# Swtich to Golang 1.13.4
git checkout -b go1.13.4 tags/go1.13.4
# Apply the patch to adapt Golang to Occlum
git apply ${THIS_DIR}/0001-adapt-golang-to-occlum-libos.patch

# Build Golang
cd src
./make.bash
mv ${BUILD_DIR} ${INSTALL_DIR}

# Generate the wrappers for Go
cat > ${INSTALL_DIR}/bin/occlum-go <<EOF
#!/bin/bash
OCCLUM_GCC="\$(which occlum-gcc)"
CC=\$OCCLUM_GCC ${INSTALL_DIR}/bin/go "\$@"
EOF

chmod +x ${INSTALL_DIR}/bin/occlum-go
