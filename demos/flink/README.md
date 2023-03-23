# Run Flink on Occlum

### Preinstall dependencies
Related dependencies: openjdk-11
```
./preinstall_deps.sh
```

### Run the flink jobmanager
```
./run_flink_jobmanager_on_host.sh
```

### Run the taskManager
```
./run_flink_on_occlum_glibc.sh tm
```

### Run flink jobs example
```
./run_flink_on_occlum_glibc.sh task
```

**Note:**  
1. If running the jobmanager in docker, please export the port 8081 and 6123
2. Step 2 may report warning for not finding shared objects. It doesn't matter. To avoid these warnings, you can **REPLACE the FIRST LINE** of config file `/opt/occlum/etc/template/occlum_elf_loader.config` with `/opt/occlum/glibc/lib/ld-linux-x86-64.so.2 /usr/lib/x86_64-linux-gnu:/lib/x86_64-linux-gnu:/usr/lib/jvm/java-11-openjdk-amd64/lib/server`.
