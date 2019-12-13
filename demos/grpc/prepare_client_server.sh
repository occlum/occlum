#!/bin/sh

cd grpc/examples/cpp/helloworld
git apply  ../../../../Makefile.patch
if [ $? -ne 0 ]
then
  echo "patch failed"
  exit 1
fi
cp ../../protos/helloworld.proto .

cd ../../../../

mkdir -p client
mkdir -p server

cp -R grpc/examples/cpp/helloworld/* client
cp -R grpc/examples/cpp/helloworld/* server

cd grpc
git checkout examples/cpp/helloworld/Makefile
cd ..

rm -rf client/cocoapods/
rm -rf client/cmake_externalproject/
rm client/CMakeLists.txt

rm -rf server/cocoapods/
rm -rf server/cmake_externalproject/
rm server/CMakeLists.txt
