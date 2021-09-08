./bin/spark-submit \
    --master k8s://https://10.239.173.66:6443 \
    --deploy-mode cluster \
    --name spark-pi \
    --class org.apache.spark.examples.SparkPi \
    --conf spark.executor.instances=1 \
	--conf spark.rpc.netty.dispatcher.numThreads=32 \
    --conf spark.kubernetes.container.image=occlum_spark:latest \
	--conf spark.kubernetes.authenticate.driver.serviceAccountName=spark \
	--conf spark.kubernetes.executor.deleteOnTermination=false \
	--conf spark.kubernetes.driver.podTemplateFile=../executor.yaml \
	--conf spark.kubernetes.executor.podTemplateFile=../executor.yaml \
    local:/bin/examples/jars/spark-examples_2.12-3.0.0.jar
    #local:/opt/spark/examples/jars/spark-examples_2.12-3.0.0.jar
    #local:/bin/examples/jars/spark-examples_2.12-3.0.0.jar
    #local:/opt/spark/examples/jars/spark-examples_2.12-3.0.0.jar
    #local:/bin/examples/jars/spark-examples_2.12-3.0.0.jar
    #local:/opt/spark/examples/jars/spark-examples_2.12-3.0.0.jar
    #--conf spark.kubernetes.container.image=occlum_spark:3.0 \
    #local:/bin/examples/jars/spark-examples_2.12-3.0.0.jar
    #--master k8s://https://10.239.173.66:6443 \
	#--conf spark.kubernetes.driver.pod.name=spark-pi-driver \
    #--conf spark.kubernetes.executor.volumes.hostPath.enclave.mount.path=/dev/sgx/enclave\
    #--conf spark.kubernetes.executor.volumes.hostPath.enclave.options.path=/dev/sgx_enclave \
    #--conf spark.kubernetes.executor.volumes.hostPath.enclave.mount.readOnly=false \
    #--conf spark.kubernetes.executor.volumes.hostPath.provision.mount.path=/dev/sgx/provision\
    #--conf spark.kubernetes.executor.volumes.hostPath.provision.options.path=/dev/sgx_provision \
    #--conf spark.kubernetes.executor.volumes.hostPath.provision.mount.readOnly=false \
    #--conf spark.kubernetes.driver.volumes.hostPath.enclave.mount.path=/dev/sgx/enclave\
    #--conf spark.kubernetes.driver.volumes.hostPath.enclave.options.path=/dev/sgx_enclave \
    #--conf spark.kubernetes.driver.volumes.hostPath.provision.mount.path=/dev/sgx/provision\
    #--conf spark.kubernetes.driver.volumes.hostPath.provision.options.path=/dev/sgx_provision \
	#--conf spark.kubernetes.executor.deleteOnTermination=false \
    #local:/opt/spark/examples/jars/spark-examples_2.12-3.0.0.jar
	#--conf spark.kubernetes.authenticate.driver.serviceAccountName=root \
