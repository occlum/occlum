fn main() {
    println!("cargo:rustc-link-search=native=../dep_libs");
    println!("cargo:rustc-link-lib=dylib=grpc_ratls_client");
    println!("cargo:rustc-link-lib=dylib=hw_grpc_proto");
}