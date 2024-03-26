#
# Copyright 2016 The BigDL Authors.
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.
#

import torch
import time
import argparse


# load Hugging Face Transformers model with INT4 optimizations
from ipex_llm.transformers import AutoModelForCausalLM
from transformers import AutoTokenizer


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description='Predict Tokens using `generate()` API for ChatGLM2 model')
    parser.add_argument('--model-path', type=str, default="THUDM/chatglm2-6b",
                        help='The original model path')
    parser.add_argument('--save-path', type=str, default="./",
                        help='The converted model save path')

    args = parser.parse_args()
    model_path = args.model_path
    save_path = args.save_path

    model = AutoModelForCausalLM.from_pretrained(model_path, trust_remote_code=True, load_in_4bit=True)
    # Load tokenizer
    tokenizer = AutoTokenizer.from_pretrained(model_path,
                                              trust_remote_code=True)
    # save model with INT4 optimizations
    model.save_low_bit(save_path)
    tokenizer.save_pretrained(save_path)
