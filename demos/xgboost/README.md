# Use XGBoost in SGX with Occlum

### Step 1: Preinstall dependencies
Related dependencies: python3-pip python3-setuptools kubernetes cmake
```
./preinstall_deps.sh
```

### Step 2: Download and build XGBoost
```
./download_and_build_xgboost.sh
```
When completed, the resulting XGBoost can be found in `xgboost_src` directory.

### Step 3: To train data with XGBoost in a single process, run
```
make test
```

### Step 4: To train data with a two-node XGBoost cluster, run
```
make test-local-cluster
```

### Step 5 (Optional): To train data with XGBoost in a single process in Linux, run
```
make test-native
```
