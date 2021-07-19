python=$1
grpc_url=$2
server_crt=$3

script_dir=$(cd "$(dirname "$0")";pwd -P)

unset http_proxy && unset https_proxy

# Batch off
$python -u $script_dir/resnet_client_grpc.py -url $grpc_url -crt $server_crt -batch 1 -cnum 1 -loop 200
#$python -u $script_dir/resnet_client_grpc.py -url $grpc_url -crt $server_crt -batch 1 -cnum 16 -loop 125
#$python -u $script_dir/resnet_client_grpc.py -url $grpc_url -crt $server_crt -batch 1 -cnum 32 -loop 100
#$python -u $script_dir/resnet_client_grpc.py -url $grpc_url -crt $server_crt -batch 1 -cnum 48 -loop 75
#$python -u $script_dir/resnet_client_grpc.py -url $grpc_url -crt $server_crt -batch 1 -cnum 64 -loop 50

# Batch on
$python -u $script_dir/resnet_client_grpc.py -url $grpc_url -crt $server_crt -cnum 1 -batch 1 -loop 100
#$python -u $script_dir/resnet_client_grpc.py -url $grpc_url -crt $server_crt -cnum 1 -batch 16 -loop 50
#$python -u $script_dir/resnet_client_grpc.py -url $grpc_url -crt $server_crt -cnum 1 -batch 32 -loop 40
#$python -u $script_dir/resnet_client_grpc.py -url $grpc_url -crt $server_crt -cnum 1 -batch 48 -loop 30
#$python -u $script_dir/resnet_client_grpc.py -url $grpc_url -crt $server_crt -cnum 1 -batch 64 -loop 20
