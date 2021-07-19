cur_dir=`pwd -P`
models_abs_dir=${cur_dir}/models
mkdir ${models_abs_dir}

# resnet50-v15
mkdir ${models_abs_dir}/resnet50-v15-fp32
cd ${models_abs_dir}/resnet50-v15-fp32
wget --no-check-certificate -c https://storage.googleapis.com/intel-optimized-tensorflow/models/v1_8/resnet50_fp32_pretrained_model.pb -O resnet50-v15-fp32.pb
