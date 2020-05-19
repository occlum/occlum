# Use Rust with Occlum

This directory contains scripts and source code to demonstrate how to
compile and run Rust programs on Occlum.

## occlum-cargo and occlum-rustc

We introduce cargo and rustc wrappers called occlum-cargo and occlum-rustc
respectively. They wrap the original commands with options specific to occlum.
Refer to tools/toolchains/rust/build.sh for more information.

## rust\_app

This directory contains source code of a Rust program with a cpp FFI. The cpp
interface increments the input by one. Rust code calls the function and
displays the result on the terminal.

One can use occlum-cargo in the way cargo is used. In the rust\_app directory,
calling ```occlum-cargo build``` will build the demo and ```occlum-cargo run```
will run the demo on host. To run the demo in occlum, run:
```
run_rust_demo_on_occlum.sh
```
The output will be displayed on the terminal:
```
5 + 1 = 6
```
