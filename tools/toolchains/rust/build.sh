#!/bin/bash
THIS_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}"  )" >/dev/null 2>&1 && pwd )"
INSTALL_DIR=/opt/occlum/toolchains/rust

mkdir -p ${INSTALL_DIR}/bin

rustup target add x86_64-unknown-linux-musl

# Generate the wrapper for Cargo
# Use -crt-static to dynamically link the application to musl libc library.
# Refer to https://github.com/rust-lang/rfcs/blob/master/text/1721-crt-static.md
# for more information about crt-static
cat > ${INSTALL_DIR}/bin/occlum-cargo <<EOF
#!/bin/bash
env CC_x86_64_unknown_linux_musl=x86_64-linux-musl-gcc \
CCFLAGS_x86_64_unknown_linux_musl="-fPIC -pie -Wl,-rpath,/opt/occlum/toolchains/gcc/x86_64-linux-musl/lib" \
CXX_x86_64_unknown_linux_musl=x86_64-linux-musl-g++ \
CXXFLAGS_x86_64_unknown_linux_musl="-fPIC -pie -Wl,-rpath,/opt/occlum/toolchains/gcc/x86_64-linux-musl/lib" \
RUSTFLAGS="-C target-feature=-crt-static -C linker=occlum-gcc" \
CARGO_BUILD_TARGET=x86_64-unknown-linux-musl \
cargo "\$@"
EOF

# Generate the wrapper for rustc
cat > ${INSTALL_DIR}/bin/occlum-rustc <<EOF
#!/bin/bash
rustc -C linker=occlum-gcc -C target-feature=-crt-static "\$@" --target=x86_64-unknown-linux-musl
EOF

chmod +x ${INSTALL_DIR}/bin/occlum-cargo
chmod +x ${INSTALL_DIR}/bin/occlum-rustc
