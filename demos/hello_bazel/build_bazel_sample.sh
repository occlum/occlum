#!/bin/bash
set -e

mkdir examples
cd examples
git clone https://github.com/bazelbuild/examples/ .
cd cpp-tutorial/stage3
export CC=/opt/occlum/toolchains/gcc/bin/occlum-gcc
export CXX=/opt/occlum/toolchains/gcc/bin/occlum-g++
bazel build --cxxopt=-std=c++11 //main:hello-world
