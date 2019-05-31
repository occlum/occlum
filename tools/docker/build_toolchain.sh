#!/bin/sh
BUILD_DIR=/root/occlum/toolchain
INSTALL_DIR=/usr/local/occlum

# Clean previous build and installation if any
rm -rf ${BUILD_DIR}
rm -rf ${INSTALL_DIR}

# Create the build directory
mkdir -p ${BUILD_DIR}
cd ${BUILD_DIR}

# Download all source code
git clone -b for_occlum https://github.com/occlum/llvm
git clone -b for_occlum https://github.com/occlum/musl
git clone -b for_occlum https://github.com/occlum/lld
git clone -b release_70 https://github.com/llvm-mirror/clang
git clone -b release_70 https://github.com/llvm-mirror/libcxx
git clone -b release_70 https://github.com/llvm-mirror/libcxxabi
git clone -b release_70 https://github.com/llvm-mirror/libunwind
git clone -b release_70 https://github.com/llvm-mirror/compiler-rt

# Build LLVM
mkdir llvm-build
cd llvm-build
cmake -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_INSTALL_PREFIX=${INSTALL_DIR} \
    -DLLVM_ENABLE_PROJECTS="clang;lld" \
    -DLLVM_TARGETS_TO_BUILD="X86" \
    ../llvm
make install
cd ..

# Make LLVM binaries visible
export PATH=${INSTALL_DIR}/bin:${PATH}

# Build musl libc
cd musl
CC=clang ./configure --prefix=${INSTALL_DIR} --enable-wrapper=clang
make install
cd ..

# Link Linux headers
ln -s /usr/include/linux ${INSTALL_DIR}/include/linux
ln -s /usr/include/asm ${INSTALL_DIR}/include/asm
ln -s /usr/include/asm-generic ${INSTALL_DIR}/include/asm-generic

# Build libunwind
mkdir libunwind-build
cd libunwind-build
cmake -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_C_COMPILER=musl-clang \
    -DCMAKE_C_FLAGS="-O2 -fPIC -locclum_stub" \
    -DCMAKE_CXX_COMPILER=musl-clang \
    -DCMAKE_CXX_FLAGS="-O2 -fPIC -locclum_stub" \
    -DCMAKE_INSTALL_PREFIX=${INSTALL_DIR} \
    -DLIBUNWIND_ENABLE_SHARED=OFF \
    -DLLVM_ENABLE_LIBCXX=ON \
    ../libunwind
make install -j
cd ..

# Build libcxx (the intermediate version)
mkdir libcxx-prebuild
cd libcxx-prebuild
cmake -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_C_COMPILER=musl-clang \
    -DCMAKE_C_FLAGS="-O2 -fPIC -locclum_stub" \
    -DCMAKE_CXX_COMPILER=musl-clang \
    -DCMAKE_CXX_FLAGS="-O2 -fPIC -locclum_stub" \
    -DCMAKE_INSTALL_PREFIX=${INSTALL_DIR} \
    -DLIBCXX_ENABLE_SHARED=OFF \
    -DLIBCXX_HAS_MUSL_LIBC=ON \
    ../libcxx
make install -j
cd ..

# Build libcxxabi with libcxx
mkdir libcxxabi-build
cd libcxxabi-build
cmake -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_C_COMPILER=musl-clang \
    -DCMAKE_C_FLAGS="-O2 -fPIC -locclum_stub" \
    -DCMAKE_CXX_COMPILER=musl-clang \
    -DCMAKE_CXX_FLAGS="-O2 -fPIC -locclum_stub" \
    -DCMAKE_INSTALL_PREFIX=${INSTALL_DIR} \
    -DLIBCXXABI_ENABLE_PIC=ON \
    -DLIBCXXABI_ENABLE_SHARED=OFF \
    -DLIBCXXABI_ENABLE_STATIC_UNWINDER=OFF \
    -DLIBCXXABI_LIBCXX_PATH=${INSTALL_DIR} \
    -DLIBCXXABI_USE_LLVM_UNWINDER=ON \
    -DLLVM_ENABLE_LIBCXX=ON \
    ../libcxxabi
make install -j
cd ..

# Build libcxx (the final version) again, but this time with the libcxxabi above
mkdir libcxx-build
cd libcxx-build
cmake -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_C_COMPILER=musl-clang \
    -DCMAKE_C_FLAGS="-O2 -fPIC -locclum_stub" \
    -DCMAKE_CXX_COMPILER=musl-clang \
    -DCMAKE_CXX_FLAGS="-O2 -fPIC -locclum_stub" \
    -DCMAKE_INSTALL_PREFIX=${INSTALL_DIR} \
    -DLIBCXX_ENABLE_SHARED=OFF \
    -DLIBCXX_HAS_MUSL_LIBC=ON \
    -DLIBCXX_CXX_ABI=libcxxabi \
    -DLIBCXX_CXX_ABI_INCLUDE_PATHS=../libcxxabi/include \
    -DLIBCXX_CXX_ABI_LIBRARY_PATH=${INSTALL_DIR}/lib \
    -DLIBCXXABI_USE_LLVM_UNWINDER=ON \
    ../libcxx
make install -j
cd ..

# Remove all source code and build files
rm -rf ${BUILD_DIR}
