#!/bin/bash
#*
#*
#* Copyright 2014 gRPC authors.
#*
#* Licensed under the Apache License, Version 2.0 (the "License");
#* you may not use this file except in compliance with the License.
#* You may obtain a copy of the License at
#*
#*     http://www.apache.org/licenses/LICENSE-2.0
#*
#* Unless required by applicable law or agreed to in writing, software
#* distributed under the License is distributed on an "AS IS" BASIS,
#* WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
#* See the License for the specific language governing permissions and
#* limitations under the License.
#*

set -e

rpcs=(1)
conns=(1)
warmup=10
dur=10
reqs=(1)
resps=(1)
rpc_types=(unary)

out_dir=$PWD/bin
# idx[0] = idx value for rpcs
# idx[1] = idx value for conns
# idx[2] = idx value for reqs
# idx[3] = idx value for resps
# idx[4] = idx value for rpc_types
idx=(0 0 0 0 0)
idx_max=(1 1 1 1 1)



# 1. Init Occlum Workspace
rm -rf occlum_server && mkdir occlum_server
rm -rf occlum_client && mkdir occlum_client
cd occlum_client
occlum init
new_json="$(jq '.resource_limits.user_space_size = "1MB" |
          .resource_limits.user_space_max_size = "2048MB" |
	        .resource_limits.max_num_of_threads = 96 ' Occlum.json)" && \
echo "${new_json}" > Occlum.json

# 2. Copy program into Occlum Workspace and build
cp ${out_dir}/client image/bin
mkdir -p image/etc
cp /etc/hosts image/etc
occlum build

cd ../occlum_server
occlum init
new_json="$(jq '.resource_limits.user_space_size = "1MB" |
          .resource_limits.user_space_max_size = "2048MB" |
	        .resource_limits.max_num_of_threads = 96 ' Occlum.json)" && \
echo "${new_json}" > Occlum.json

# 2. Copy program into Occlum Workspace and build
cp ${out_dir}/server image/bin
mkdir -p image/etc
cp /etc/hosts image/etc
occlum build


inc()
{
  for i in $(seq $((${#idx[@]}-1)) -1 0); do
    idx[${i}]=$((${idx[${i}]}+1))
    if [ ${idx[${i}]} == ${idx_max[${i}]} ]; then
      idx[${i}]=0
    else
      break
    fi
  done
  local fin
  fin=1
  # Check to see if we have looped back to the beginning.
  for v in ${idx[@]}; do
    if [ ${v} != 0 ]; then
      fin=0
      break
    fi
  done
  if [ ${fin} == 1 ]; then
    clean_and_die 0
  fi
}

clean_and_die() {
  exit $1
}


run(){
  local nr
  nr=${rpcs[${idx[0]}]}
  local nc
  nc=${conns[${idx[1]}]}
  req_sz=${reqs[${idx[2]}]}
  resp_sz=${resps[${idx[3]}]}
  r_type=${rpc_types[${idx[4]}]}
  # Following runs one benchmark
  base_port=50051
  delta=0
  test_name="r_"${nr}"_c_"${nc}"_req_"${req_sz}"_resp_"${resp_sz}"_"${r_type}"_"$(date +%s)
  echo "================================================================================"
  echo ${test_name}
  while :
  do
    port=$((${base_port}+${delta}))

    # Launch the server in background
    occlum run /bin/server --port=${port} --test_name="Server_"${test_name}&
    sleep 1
    server_pid=`ps -a | grep occlum-run | grep -v grep | sed 's/^\s*//' | sed 's/[[:space:]].*//'`
    # Launch the client
    cd ../occlum_client
    occlum run /bin/client --port=${port} --d=${dur} --w=${warmup} --r=${nr} --c=${nc} --req=${req_sz} --resp=${resp_sz} --rpc_type=${r_type}  --test_name="client_"${test_name}
    client_status=$(echo $?)
    echo "client_status=$client_status"

    #occlum app cannot capture the interrupt, kill the server directly.
    #kill -INT ${server_pid}
    kill -9 ${server_pid}
    #wait ${server_pid}

    if [ ${client_status} == 0 ]; then
      break
    fi

    delta=$((${delta}+1))
    if [ ${delta} == 10 ]; then
      echo "Continuous 10 failed runs. Exiting now."
    fi
  done

}

set_param(){
  local argname=$1
  shift
  local idx=$1
  shift
  if [ $# -eq 0 ]; then
    echo "${argname} not specified"
    exit 1
  fi
  PARAM=($(echo $1 | sed 's/,/ /g'))
  if [ ${idx} -lt 0 ]; then
    return
  fi
  idx_max[${idx}]=${#PARAM[@]}
}

while [ $# -gt 0 ]; do
  case "$1" in
    -r)
      shift
      set_param "number of rpcs" 0 $1
      rpcs=(${PARAM[@]})
      shift
      ;;
    -c)
      shift
      set_param "number of connections" 1 $1
      conns=(${PARAM[@]})
      shift
      ;;
    -w)
      shift
      set_param "warm-up period" -1 $1
      warmup=${PARAM}
      shift
      ;;
    -d)
      shift
      set_param "duration" -1 $1
      dur=${PARAM}
      shift
      ;;
    -req)
      shift
      set_param "request size" 2 $1
      reqs=(${PARAM[@]})
      shift
      ;;
    -resp)
      shift
      set_param "response size" 3 $1
      resps=(${PARAM[@]})
      shift
      ;;
    -rpc_type)
      shift
      set_param "rpc type" 4 $1
      rpc_types=(${PARAM[@]})
      shift
      ;;
    -h|--help)
      echo "Following are valid options:"
      echo
      echo "-h, --help        show brief help"
      echo "-w                warm-up duration in seconds, default value is 10"
      echo "-d                benchmark duration in seconds, default value is 60"
      echo ""
      echo "Each of the following can have multiple comma separated values."
      echo ""
      echo "-r                number of RPCs, default value is 1"
      echo "-c                number of Connections, default value is 1"
      echo "-req              req size in bytes, default value is 1"
      echo "-resp             resp size in bytes, default value is 1"
      echo "-rpc_type         valid values are unary|streaming, default is unary"
      exit 0
      ;;
    *)
      echo "Incorrect option $1"
      exit 1
      ;;
  esac
done

# Build server and client
if [ $? != 0 ]; then
  clean_and_die 1
fi

#occlum start
while :
do
  run
  inc
done
