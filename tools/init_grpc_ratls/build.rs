fn main() {
    println!("cargo:rustc-link-search=native=/opt/occlum/toolchains/dcap_lib/musl");
    println!("cargo:rustc-link-search=native=/opt/occlum/toolchains/grpc_ratls/musl");
    println!("cargo:rustc-link-lib=dylib=grpc_ratls_client");
    println!("cargo:rustc-link-lib=dylib=hw_grpc_proto");
    println!("cargo:rustc-link-lib=dylib=occlum_dcap");
}
