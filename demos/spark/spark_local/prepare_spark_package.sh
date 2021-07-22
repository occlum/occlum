wget https://archive.apache.org/dist/spark/spark-3.0.0/spark-3.0.0-bin-hadoop2.7.tgz
tar -xvzf spark-3.0.0-bin-hadoop2.7.tgz
cp spark-network-common_2.12-3.0.0.jar spark-3.0.0-bin-hadoop2.7/jars/
apt-get update
apt-get install -y openjdk-11-jdk

