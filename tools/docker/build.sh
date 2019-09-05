#!/bin/bash
script_dir="$( cd "$( dirname "${BASH_SOURCE[0]}"  )" >/dev/null 2>&1 && pwd )"

set -e

if [[ ( "$#" < 1 ) ]] ; then
    echo "Error: tag is not given"
    echo ""
    echo "Usage: run command"
    echo "    build.sh <tag>"
    echo "to build a Docker image with a tag (e.g., occlum/occlum:latest)."
    exit 1
fi
tag=$1

cd "$script_dir/.."
docker build -f "$script_dir/Dockerfile" -t "$tag" .
