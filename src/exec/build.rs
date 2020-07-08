extern crate protoc_rust_grpc;

fn main() {
    protoc_rust_grpc::Codegen::new()
        .out_dir("src")
        .input("occlum_exec.proto")
        .rust_protobuf(true)
        .run()
        .expect("protoc-rust-grpc");

    println!("cargo:rustc-link-search=native=../../build/lib");
    println!("cargo:rustc-link-lib=dylib=occlum-pal");
}
