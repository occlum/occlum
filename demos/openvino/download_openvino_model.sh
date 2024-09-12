#!/bin/bash
set -e

rm -rf model && mkdir model
pushd model
wget https://storage.openvinotoolkit.org/repositories/open_model_zoo/2023.0/models_bin/1/age-gender-recognition-retail-0013/FP16/age-gender-recognition-retail-0013.xml
wget https://storage.openvinotoolkit.org/repositories/open_model_zoo/2023.0/models_bin/1/age-gender-recognition-retail-0013/FP16/age-gender-recognition-retail-0013.bin
popd
