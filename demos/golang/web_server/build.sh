#!/bin/bash
set -e

rm -f go.mod
occlum-go mod init web_server
occlum-go mod tidy

occlum-go build -o web_server ./web_server.go
