1. Update system time and install kubernetes
```
date -s ${year}/${month}/${day}
date -s ${hour}:${minute}:${second}

./install_kubernetes.sh
```

2. Init and enable taint for master node
```
unset http_proxy && unset https_proxy

swapoff -a && free -m
kubeadm init --v=5 --node-name=master-node --pod-network-cidr=10.244.0.0/16

mkdir -p $HOME/.kube
sudo cp -i /etc/kubernetes/admin.conf $HOME/.kube/config
sudo chown $(id -u):$(id -g) $HOME/.kube/config

kubectl taint nodes --all node-role.kubernetes.io/master-
```

3. Setup flannel network service
> flannel/readme.txt

4. Setup ingress-nginx service
> ingress-ngnix/readme.txt
