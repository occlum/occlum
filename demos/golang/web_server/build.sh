#!/bin/bash
set -e

rm -f go.mod
occlum-go mod init web_server
occlum-go mod tidy
occlum-go get -u -v github.com/gin-gonic/gin
occlum-go get -u -v golang.org/x/crypto@v0.23.0

occlum-go build -o web_server ./web_server.go
