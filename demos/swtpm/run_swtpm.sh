#!/bin/bash
set -e

BLUE='\033[1;34m'
NC='\033[0m'

[ -d /bin/myvtpm ] ||  mkdir /bin/myvtpm
cd occlum_instance && rm -rf image
copy_bom -f ../swtpm.yaml --root image --include-dir /opt/occlum/etc/template

new_json="$(jq '.resource_limits.user_space_size = "800MB" |
        .resource_limits.kernel_space_heap_size = "600MB"|
        .env.default += ["LD_LIBRARY_PATH=/bin/:/opt/occlum/glibc/lib/"] ' Occlum.json)" && \
echo "${new_json}" > Occlum.json

# Build Occlum
echo -e "${BLUE}Occlum build swtpm${NC}"
occlum build

# Run the python demo
echo -e "${BLUE}Occlum start swtpm${NC}"

occlum run /bin/swtpm socket --tpmstate dir=/bin/myvtpm --tpm2 --ctrl type=tcp,port=2322,bindaddr=0.0.0.0 --server type=tcp,port=2321,bindaddr=0.0.0.0 --flags not-need-init --seccomp action=none

