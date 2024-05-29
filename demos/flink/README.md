# Run Flink on Occlum

This is for how to run Flink job manager and task manager in Occlum.
For how to start Flink K8S cluster in Occlum, please refer to [kubernetes](./kubernetes/).

### Preinstall dependencies
Related dependencies: openjdk-11
```
./preinstall_deps.sh
```

### Download flink
```
./download_flink.sh
```

### Build Occlum instance
```
./build_occlum_instance.sh
```

### Run flink job manager on Occlum
```
./run_flink_on_occlum.sh jm
```

Wait a while for job manager started successfully. You can check the log `occlum_instance_jobmanager/flink--standalonesession-0.log` for detail status.

### Run flink task manager on Occlum

Once the job manager is up, you can run the task manager.
```
./run_flink_on_occlum.sh tm
```

Wait a while for task manager started successfully. You can check the log `occlum_instance_taskmanager/flink--taskmanager-0.log` for detail status.

### Submit a flink job to occlum

You can submit an example flink job by using the following command:
```
./run_flink_on_occlum.sh task
```

**Note:**  
If running the jobmanager in docker, please export the port 8081 and 6123.
