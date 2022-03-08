apt-get update
apt-get install -y openjdk-11-jre
#The openjdk has a broken symlink, remove it as a workaround
rm /usr/lib/jvm/java-11-openjdk-amd64/lib/security/blacklisted.certs

wget https://archive.apache.org/dist/flink/flink-1.10.1/flink-1.10.1-bin-scala_2.11.tgz
tar -xvzf flink-1.10.1-bin-scala_2.11.tgz
