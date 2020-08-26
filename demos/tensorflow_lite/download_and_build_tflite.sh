#!/bin/bash
set -e
SRC_DIR="tensorflow_src"

# 1. Build tensorflow lite and the demo program
mkdir -p $SRC_DIR
pushd $SRC_DIR
git clone https://github.com/tensorflow/tensorflow .
git checkout tags/v1.15.0-rc0 -b v1.15.0-rc0
git apply ../patch/fix-tflite-Makefile-v1.15.0-rc0.diff
# Fix eigen source package url failure
sed -i '32 s/.*/EIGEN_URL="https:\/\/storage.googleapis.com\/mirror.tensorflow.org\/bitbucket.org\/eigen\/eigen\/get\/49177915a14a.tar.gz"/' tensorflow/lite/tools/make/download_dependencies.sh
./tensorflow/lite/tools/make/download_dependencies.sh
make -j 3 -f tensorflow/lite/tools/make/Makefile
popd

# 2. Download tflite model and labels
mkdir models
curl https://storage.googleapis.com/download.tensorflow.org/models/mobilenet_v1_2018_02_22/mobilenet_v1_1.0_224.tgz | tar xzv -C ./models
curl https://storage.googleapis.com/download.tensorflow.org/models/mobilenet_v1_1.0_224_frozen.tgz  | tar xzv -C ./models  mobilenet_v1_1.0_224/labels.txt
mv ./models/mobilenet_v1_1.0_224/labels.txt ./models/labels.txt
rm -rf ./models/mobilenet_v1_1.0_224
