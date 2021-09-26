./run_occlum_redis.sh &
sleep 20
echo 'start client'
/usr/local/occlum/x86_64-linux-musl/redis/bin/redis-benchmark -n 1000
