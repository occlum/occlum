#!/bin/bash
set -e

BLUE='\033[1;34m'
NC='\033[0m'

# Install SQLite with occlum-go
occlum-go mod init simple_demo_instance && \
occlum-go get -u -v github.com/mattn/go-sqlite3

# Build the Golang SQLite demo program using the Occlum Golang toolchain (i.e., occlum-go)
occlum-go build -o simple_demo simple_demo.go

# Init Occlum Workspace
rm -rf simple_demo_instance && mkdir simple_demo_instance
cd simple_demo_instance
occlum init
new_json="$(jq '.resource_limits.user_space_size = "2560MB" |
	.resource_limits.kernel_space_heap_size="320MB" |
	.resource_limits.kernel_space_stack_size="10MB" |
	.process.default_stack_size = "40MB" |
	.process.default_heap_size = "320MB" |
	.process.default_mmap_size = "960MB" ' Occlum.json)" && \
echo "${new_json}" > Occlum.json

# Copy program into Occlum Workspace and build
rm -rf image && \
copy_bom -f ../go_sqlite.yaml --root image --include-dir /opt/occlum/etc/template && \
occlum build

# Run the Golang SQLite demo
echo -e "${BLUE}occlum run /bin/simple_demo${NC}"
time occlum run /bin/simple_demo
