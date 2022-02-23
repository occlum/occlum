#!/bin/bash

# Users should run this script in secure environment and keep
# the geneated key/cert proctected.

pushd ~
openssl rand -writerand .rnd
popd

# Geneate self-signed key/cert
# Generate valid Flask server Key/Cert
openssl genrsa -out flask.key 2048
openssl req -nodes -new -key flask.key -subj "/CN=localhost" -out flask.csr
openssl x509 -req -sha256 -days 365 -in flask.csr -signkey flask.key -out flask.crt
