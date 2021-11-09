# A Sample C++ Project with Makefile and CMake

This project demonstrates how to use Makefile/CMake to build C++ projects for Occlum.

1. Build `hello_world` with Makefile
```
make
```
Or you can build `hello_world` with CMake
```
mkdir build && cd build
cmake ../ -DCMAKE_CXX_COMPILER=occlum-g++
make
cd ..
cp build/hello_world .
```
Either way, the resulting `hello_world` can be found in the current directory.

2. (Optional) Run `hello_world` on Linux
```
./hello_world
```

3. Run `hello_world` on Occlum
```
mkdir occlum_workspace && cd occlum_workspace
occlum init && rm -rf image
copy_bom -f ../hello.yaml --root image --include-dir /opt/occlum/etc/template
occlum build
occlum run /bin/hello_world
```
