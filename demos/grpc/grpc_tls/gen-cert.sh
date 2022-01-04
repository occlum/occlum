#!/bin/bash

pushd ~
openssl rand -writerand .rnd
popd

# Generate valid CA
openssl genrsa -out ca.key 4096
openssl req -new -x509 -days 365 -key ca.key -out ca.crt -subj  "/OU=Test/CN=Root CA"

# Generate valid Server Key/Cert
openssl genrsa -out server.key 4096
openssl req -new -key server.key -out server.csr -subj  "/OU=Server/CN=localhost"
openssl x509 -req -days 365 -in server.csr -CA ca.crt -CAkey ca.key -set_serial 01 -out server.crt

# Remove passphrase from the Server Key
openssl rsa -in server.key -out server.key

# Generate valid Client Key/Cert
openssl genrsa -out client.key 4096
openssl req -new -key client.key -out client.csr -subj  "/OU=Client/CN=localhost"
openssl x509 -req -days 365 -in client.csr -CA ca.crt -CAkey ca.key -set_serial 01 -out client.crt

# Remove passphrase from Client Key
openssl rsa -in client.key -out client.key

