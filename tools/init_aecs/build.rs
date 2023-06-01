fn main() {
    println!("cargo:rustc-link-search=native=/opt/occlum/toolchains/aecs_client");
    println!("cargo:rustc-link-lib=dylib=aecs_client");
    println!("cargo:rustc-link-lib=dylib=ual");
    println!("cargo:rustc-link-lib=curl_static");
    println!("cargo:rustc-link-lib=dylib=ssl");
    println!("cargo:rustc-link-lib=dylib=z");
    println!("cargo:rustc-link-lib=dylib=crypto");
}
