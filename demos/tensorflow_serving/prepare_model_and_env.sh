apt-get update
apt install -y python3-pip 
pip3 install --upgrade pip
pip3 install --upgrade tensorflow==2.4
./download_model.sh
python3 ./model_graph_to_saved_model.py --import_path ./models/resnet50-v15-fp32/resnet50-v15-fp32.pb --export_dir ./resnet50-v15-fp32 --model_version 1 --inputs input --outputs predict
./generate_ssl_config.sh localhost

