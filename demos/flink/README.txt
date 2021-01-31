1. Run the flink jobmanager
    ./run_flink_jobmanager_on_host.sh
2. Run the taskManager
    ./run_flink_on_occlum_glibc.sh tm
3. Run flink jobs example
    ./run_flink_on_occlum.sh task

Note: If running the jobmanager in docker, please export the port 8081 and 6123
