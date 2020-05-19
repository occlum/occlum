extern crate cc;

fn main() {
    cc::Build::new()
        .file("src/util.cpp")
        .cpp(true)
        .compile("libutil.a");
}
