extern crate protoc_rust_grpc;
use std::env;

fn main() {
    protoc_rust_grpc::Codegen::new()
        .out_dir("src")
        .input("occlum_exec.proto")
        .rust_protobuf(true)
        .run()
        .expect("protoc-rust-grpc");

    let sdk_dir = env::var("SGX_SDK").unwrap_or_else(|_| "/opt/intel/sgxsdk".to_string());
    let sgx_mode = env::var("SGX_MODE").unwrap_or_else(|_| "HW".to_string());
    match sgx_mode.as_ref() {
        "SW" | "SIM" => {
            println!("cargo:rustc-link-search=native={}/sdk_libs", sdk_dir);
            println!("cargo:rustc-link-search=native=../../build_sim/lib");
            println!("cargo:rustc-link-lib=dylib=sgx_uae_service_sim");
            println!("cargo:rustc-link-lib=dylib=sgx_urts_sim")
        }
        "HW" | _ => println!("cargo:rustc-link-search=native=../../build/lib"), // Treat undefined as HW
    }
    println!("cargo:rustc-link-lib=dylib=occlum-pal");
}
