#run the occlum benchmark
./run_occlum_redis_glibc.sh &
sleep 20
echo 'start client'
/usr/local/redis/bin/redis-benchmark -n 1000
