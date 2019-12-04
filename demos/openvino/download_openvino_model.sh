#!/bin/bash
set -e

rm -rf model && mkdir model
cd model
wget https://download.01.org/opencv/2019/open_model_zoo/R3/20190905_163000_models_bin/age-gender-recognition-retail-0013/FP32/age-gender-recognition-retail-0013.bin
wget https://download.01.org/opencv/2019/open_model_zoo/R3/20190905_163000_models_bin/age-gender-recognition-retail-0013/FP32/age-gender-recognition-retail-0013.xml
