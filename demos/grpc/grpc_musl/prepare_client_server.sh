#!/bin/sh
DEMO_DIR=$PWD

cd $DEMO_DIR/grpc/examples/cpp/helloworld
git apply $DEMO_DIR/Makefile.patch
if [ $? -ne 0 ]
then
  echo "patch failed"
  exit 1
fi

cp $DEMO_DIR/grpc/examples/protos/helloworld.proto .

cd $DEMO_DIR

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
