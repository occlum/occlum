#Run tensorflow_serving in Occlum
1. ./prepare_model_and_env.sh
2. ./run_occlum_tf_serving.sh
#Run client benchmark.
3. cd client
4. ./prepare_client_env.sh
5. ./benchmark.sh python3 localhost:8500 ../ssl_configure/server.crt 
