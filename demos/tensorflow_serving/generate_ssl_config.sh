service_domain_name=$1

rm -rf ssl_configure
mkdir ssl_configure
cd ssl_configure

# https://kubernetes.github.io/ingress-nginx/examples/PREREQUISITES/#client-certificate-authentication
openssl req -x509 -sha256 -nodes -days 365 -newkey rsa:2048 -keyout server.key -out server.crt -subj "/CN=${service_domain_name}"

# Generate tls configure
## https://stackoverflow.com/questions/59199419/using-tensorflow-model-server-with-ssl-configuration

echo "server_key: '`cat server.key | paste -d "" -s`'" >> ssl.cfg
echo "server_cert: '`cat server.crt | paste -d "" -s`'" >> ssl.cfg
echo "client_verify: false" >> ssl.cfg

sed -i "s/-----BEGIN PRIVATE KEY-----/-----BEGIN PRIVATE KEY-----\\\n/g" ssl.cfg
sed -i "s/-----END PRIVATE KEY-----/\\\n-----END PRIVATE KEY-----/g" ssl.cfg
sed -i "s/-----BEGIN CERTIFICATE-----/-----BEGIN CERTIFICATE-----\\\n/g" ssl.cfg
sed -i "s/-----END CERTIFICATE-----/\\\n-----END CERTIFICATE-----/g" ssl.cfg

echo "Generate server.key server.crt and ssl.cfg successfully!"
#cat ssl.cfg
cd -
