#!/bin/bash

pushd ~
openssl rand -writerand .rnd
popd

# Generate self-signed key/cert
# Generate valid Flask server Key/Cert
openssl genrsa -out flask.key 2048
openssl req -nodes -new -key flask.key -subj "/CN=localhost" -out flask.csr
openssl x509 -req -sha256 -days 365 -in flask.csr -signkey flask.key -out flask.crt

# Remove passphrase from the Key
openssl rsa -in flask.key -out flask.key


