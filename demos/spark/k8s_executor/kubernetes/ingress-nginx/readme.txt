# https://raw.githubusercontent.com/kubernetes/ingress-nginx/master/deploy/static/provider/baremetal/deploy.yaml
# https://kubernetes.github.io/ingress-nginx/deploy/#bare-metal

# Deploy service
kubectl apply -f ./deploy-nodeport.yaml

# Delete service
kubectl delete -f ./deploy-nodeport.yaml

# Check status
kubectl get -n ingress-nginx service/ingress-nginx-controller -o yaml
kubectl get -n ingress-nginx deployment.apps/ingress-nginx-controller -o yaml