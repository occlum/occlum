import argparse
import torch
from ipex_llm.transformers import AutoModel, AutoModelForCausalLM
from transformers import AutoTokenizer
from benchmark_util import BenchmarkWrapper

parser = argparse.ArgumentParser(description='Predict Tokens using `generate()` API for ChatGLM2 model')
parser.add_argument('--repo-id-or-model-path', type=str, default="THUDM/chatglm2-6b",
                    help='The huggingface repo id for the ChatGLM2 model to be downloaded'
                            ', or the path to the huggingface checkpoint folder')

args = parser.parse_args()
model_path = args.repo_id_or_model_path
model = AutoModelForCausalLM.from_pretrained(model_path, trust_remote_code=True, load_in_4bit=True)
model = BenchmarkWrapper(model, do_print=True)
tokenizer = AutoTokenizer.from_pretrained(model_path, trust_remote_code=True)
prompt = "今天睡不着怎么办"
 
with torch.inference_mode():
    input_ids = tokenizer.encode(prompt, return_tensors="pt")
    output = model.generate(input_ids, do_sample=False, max_new_tokens=512)
    output_str = tokenizer.decode(output[0], skip_special_tokens=True)