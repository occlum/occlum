extern crate protoc_rust_grpc;

const PROTO_FILE: &str = "occlum_exec.proto";

fn main() {
    protoc_rust_grpc::Codegen::new()
        .out_dir("src")
        .input(PROTO_FILE)
        .rust_protobuf(true)
        .run()
        .expect("protoc-rust-grpc");

    println!("cargo:rerun-if-changed={}", PROTO_FILE);
    println!("cargo:rustc-link-search=native=../../build/lib");
    println!("cargo:rustc-link-lib=dylib=occlum-pal");
}
