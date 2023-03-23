## Run Filebench on Occlum

[Filebench](https://github.com/Filebench/Filebench) is a benchmark tool aiming to test file system and storage under certain workloads. This demo demonstrates how can Filebench run on Occlum, guarded with Intel SGX.

### Step 1: Preinstall dependencies
Related dependencies: bison flex
```
./preinstall_deps.sh
```

### Step 2: Build Filebench from source
```
cd demos/Filebench && ./dl_and_build_Filebench.sh
```

The script will download source code, make some adaptation then compile Filebench into a binary.

### Step 3: Run Filebench workloads
```
cd demos/Filebench && ./run_workload.sh
```

The script will run user-specific workloads under `Filebench/workloads`. The corresponding results will be outputed.

Refer to [Filebench/wiki/Workload-model-language](https://github.com/Filebench/Filebench/wiki/Workload-model-language) and see more information about workloads.
