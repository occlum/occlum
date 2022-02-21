#!/bin/bash
set -e

BLUE='\033[1;34m'
NC='\033[0m'

# Run the python demo
cd occlum_instance
echo -e "${BLUE}occlum run /bin/rest_api.py${NC}"
occlum run /bin/rest_api.py
