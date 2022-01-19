apt-get update
apt-get install -y openjdk-11-jdk
[ -f flink-1.11.3-bin-scala_2.11.tgz ] || wget https://archive.apache.org/dist/flink/flink-1.11.3/flink-1.11.3-bin-scala_2.11.tgz
[ -d flink-1.11.3 ] || tar -xvzf flink-1.11.3-bin-scala_2.11.tgz
