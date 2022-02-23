#!/bin/bash
set -e

echo "Start GRPC server on backgound ..."

pushd occlum_server
occlum run /bin/server &
popd

sleep 3

echo "Start Flask-TLS restful web portal on backgound ..."

pushd occlum_client
occlum run /bin/rest_api.py &
popd