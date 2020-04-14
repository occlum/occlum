# Use XGBoost in SGX with Occlum

Step 1: Download XGBoost and its dependencies, and then build XGBoost
```
./download_and_build_xgboost.sh
```
When completed, the resulting XGBoost can be found in `xgboost_src` directory.

Step 2: To train data with XGBoost in a single process, run
```
make test
```

Step 3: To train data with a two-node XGBoost cluster, run
```
make test-local-cluster
```

Step 4 (Optional): To train data with XGBoost in a single process in Linux, run
```
make test-native
```
