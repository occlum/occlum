## Spark 2.4.3 local test:
```
1. enter the 0.23 occlum container
2.apt-get update &&/
  apt-get install -y openjdk-11-jdk
3. wget https://archive.apache.org/dist/spark/spark-2.4.3/spark-2.4.3-bin-hadoop2.7.tgz
4. tar -xvzf spark-2.4.3-bin-hadoop2.7.tgz
5. replace the spark-network-common_2.11-2.4.3.jar
6. ./run_spark_on_occlum_glibc.sh test
```

### Enable Spark 3.0.0 in occlum with K8S

## Setup Kubernetes cluster:
1. First, please make sure the system time in your machine is the latest, if not, please update it.
2. Install Kubernetes from [wiki](https://kubernetes.io/zh/docs/setup/production-environment) or following commands:
```
cd <graphene repository>/Examples/tensorflow-serving-cluster/kubernetes
./install_kubernetes.sh
```
3. Initialize and enable taint for master node: 
```
unset http_proxy && unset https_proxy

swapoff -a && free -m
kubeadm init --v=5 --node-name=master-node --pod-network-cidr=10.244.0.0/16

mkdir -p $HOME/.kube
sudo cp -i /etc/kubernetes/admin.conf $HOME/.kube/config
sudo chown $(id -u):$(id -g) $HOME/.kube/config

kubectl taint nodes --all node-role.kubernetes.io/master-
```
4. Deploy flannel and ingress-nginx service:
```
kubectl apply -f flannel/deploy.yaml
kubectl apply -f ingress-nginx/deploy.yaml
```
5. Add the spark account
```
kubectl create serviceaccount spark
kubectl create clusterrolebinding spark-role --clusterrole=edit --serviceaccount=default:spark --namespace=default
```
## Run Spark executor in Occlum:
```
1. wget https://archive.apache.org/dist/spark/spark-3.0.0/spark-3.0.0-bin-hadoop2.7.tgz
2. tar -xvzf spark-3.0.0-bin-hadoop2.7.tgz
3. replace the spark-network-common_2.12-3.0.0.jar
4. ./build_image.sh 3.0
5. cp run_pi.sh spark-3.0.0-bin-hadoop2.7/
6. Modify "--master k8s://https://10.239.173.66:6443" to your k8s master url in the run_pi.sh 
7. Modify "spark.kubernetes.driver.podTemplateFile" and "spark.kubernetes.executor.podTemplateFil" in run_pi.sh
8. ./run_pi.sh
```
