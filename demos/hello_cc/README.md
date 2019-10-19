# A Sample C++ Project with Makefile and CMake

This project demonstrates how to use Makefile/CMake to build C++ projects for Occlum.

1. Build `hello_world` with Makefile
```
make
```
Or you can build `hello_world` with CMake
```
mkdir build && cd build
cmake ../ -DCMAKE_CXX_COMPILER=occlum-g++ -DCMAKE_CXX_FLAGS="-fPIC -pie"
make
cd ..
cp build/hello_world .
```
Either way, the resulting `hello_world` can be found in the current directory.

2. (Optional) Run `hello_world` on Linux
```
LD_LIBRARY_PATH=/usr/local/occlum/x86_64-linux-musl/lib ./hello_world
```

3. Run `hello_world` on Occlum
```
mkdir occlum_workspace && cd occlum_workspace
occlum init
cp ../hello_world image/bin
occlum build
occlum run /bin/hello_world
```
