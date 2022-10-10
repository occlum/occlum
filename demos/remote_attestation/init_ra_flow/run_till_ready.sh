#!/bin/bash
set -e

GRPC_SERVER_IP=localhost
GRPC_SERVER_PORT=50051
FLASK_SERVER_IP=localhost
FLASK_SERVER_PORT=4996

echo "Start GRPC server on backgound ..."

pushd occlum_server
occlum run /bin/server "${GRPC_SERVER_IP}:${GRPC_SERVER_PORT}" &
popd

while ! nc -z $GRPC_SERVER_IP $GRPC_SERVER_PORT; do
  sleep 1
done

echo "Start Flask-TLS restful web portal on backgound ..."

pushd occlum_client
occlum run --config-svn 1234 /bin/rest_api.py &
popd

while ! nc -z $FLASK_SERVER_IP $FLASK_SERVER_PORT; do
  sleep 1
done
