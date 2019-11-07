#!/bin/bash

RED='\033[1;36m'
NC='\033[0m'

# Create a temporal folder and run xgboost demo
tmp_dir="tmp_$RANDOM"
mkdir -p $tmp_dir
cp -a occlum_workspace/. $tmp_dir

cd $tmp_dir
echo -e "${RED}occlum run xgboost $@${NC}"
occlum run /bin/xgboost /data/mushroom.conf $@
