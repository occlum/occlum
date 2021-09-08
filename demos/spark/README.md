## Spark 3.0.0 local test:
```
1. ./prepare_spark_package.sh
2. ./run_spark_on_occlum_glibc.sh test
```

### Enable Spark 3.0.0 in occlum with K8S

## Setup Kubernetes cluster:
1. First, please make sure the system time in your machine is the latest, if not, please update it.
2. Install Kubernetes from [wiki](https://kubernetes.io/zh/docs/setup/production-environment) or following commands:
```
cd k8s_executor/kubernetes/
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
1. ./prepare_spark_package.sh
2. ./build_image.sh 3.0
3. cp run_pi.sh spark-3.0.0-bin-hadoop2.7/
4. Modify "--master k8s://https://10.239.173.66:6443" to your k8s master url in the run_pi.sh 
5. Modify "spark.kubernetes.driver.podTemplateFile" and "spark.kubernetes.executor.podTemplateFil" in run_pi.sh
6. ./run_pi.sh
```
