# Analytics Zoo Cluster Serving Inference in SGX with Occlum #

This example demonstrates how to use Analytics Zoo Cluster Serving for real-time inference in SGX. 
[Analytics Zoo](https://github.com/intel-analytics/analytics-zoo) is an open source Big Data AI platform, [Cluster Serving](https://www.usenix.org/conference/opml20/presentation/song) is a real-time serving solution that enables automatic model inference on Flink cluster.

Note that in this example all components are run on single machine within one container. For running cluster serving with SGX on multi-nodes, please refer to [distributed mode guide](https://github.com/intel-analytics/analytics-zoo/tree/master/ppml/trusted-realtime-ml/scala/docker-occlum#distributed-mode-multi-containersmulti-nodes) from Analytics Zoo. 

Besides following steps in this demo, user can also choose to directly use the docker image provided by Analytics Zoo for cluster serving with Occlum which has all dependencies pre-installed. For detailed guide using the docker image,  please refer to [Analytics Zoo guide](https://analytics-zoo.readthedocs.io/en/latest/doc/PPML/Overview/ppml.html#trusted-realtime-compute-and-ml).

## Set up environment ##
Set environment variables and install dependencies (Redis, Flink, Analytics Zoo, models)

	source ./environment.sh
    ./install-dependencies.sh
## Start Cluster Serving ##
Start Redis, Flink and cluster serving

	./start-all.sh
Or you can start components separately:


1. **Start Redis Server**

    `./start-redis.sh &`


2. **Start Flink**

	Start Flink Jobmanager on host

	`./start-flink-jobmanager.sh`

	Initialize and start Flink Taskmanager with Occlum
	
	``` 
	./init-occlum-taskmanager.sh  			
	./start-flink-taskmanager.sh
	```
   
3. **Start Cluster Serving job**

	Start HTTP frontend
	
	`./start-http-frontend.sh &`
    
	Start cluster serving job
	
	`./start-cluster-serving-job.sh`

## Push inference image ##
Push image into queue via Restful API for inference. Users can modify the script with base64 of inference image (note that the image size must match model input size, e.g. 224*224 for resnet50 in this demo). Users can also use python API to directly push the image file, see [guide](https://analytics-zoo.github.io/master/#ClusterServingGuide/ProgrammingGuide/#4-model-inference) for details.

    ./push-image.sh
## Stop Cluster Serving ##
Stop cluster serving job and all components
	
	./stop-all.sh