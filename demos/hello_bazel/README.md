# A Sample C++ Project with Bazel

This project demonstrates how to use Bazel to build C++ projects for Occlum. To install Bazel on Ubuntu, follow the instructions [here](https://docs.bazel.build/versions/master/install-ubuntu.html).

1. Download a Bazel sample project in C++ and build it with Occlum toolchain
```
./build_bazel_sample.sh
```
When completed, the resulting `hello-world` can be found in `examples/cpp-tutorial/stage3/bazel-bin/main` directory.

2. (Optional) Run `hello-world` on Linux
```
./examples/cpp-tutorial/stage3/bazel-bin/main/hello-world
```

3. Run `hello-world` on Occlum
```
mkdir occlum_workspace && cd occlum_workspace
occlum init
cp ../examples/cpp-tutorial/stage3/bazel-bin/main/hello-world image/bin
occlum build
occlum run /bin/hello-world
```
