## Run Filebench on Occlum

[Filebench](https://github.com/Filebench/Filebench) is a benchmark tool aiming to test the file system and the storage system under certain workloads. This demo demonstrates how can Filebench run on Occlum.

### Step 1: Preinstall dependencies
Related dependencies: bison flex
```
cd demos/benchmarks/filebench && ./preinstall_deps.sh
```

### Step 2: Build Filebench from source
```
cd demos/benchmarks/filebench && ./dl_and_build_Filebench.sh
```

The script will download the source code, make some adaptation then compile Filebench into a binary.

### Step 3: Run Filebench workloads
```
cd demos/benchmarks/filebench && ./run_workload.sh <workload_name>
```

The script will run user-specific workloads under `filebench/workloads`. The corresponding results will be outputed.

Refer to [Filebench/wiki/Workload-model-language](https://github.com/Filebench/Filebench/wiki/Workload-model-language) and see more information about workloads.
