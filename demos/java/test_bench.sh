#!/bin/bash
set -ex

if [ ! -d "occlum_instance" ]; then
 echo "Error: occlum_instance directory not found."
 exit 1
fi

if [ $# -eq 0 ]; then
 echo "Error: please specify the number of repetitions."
 exit1 
fi
N=$1

cd occlum_instance
#occlum build -f
occlum start

start_time=$(date +%s.%N)
for i in $(seq 1 $N); do
 occlum exec /usr/lib/jvm/java-11-alibaba-dragonwell/jre/bin/java -Xmx512m -XX:-UseCompressedOops -XX:MaxMetaspaceSize=64m -Dos.name=Linux Main
done
end_time=$(date +%s.%N)

total_time=$(echo "$end_time - $start_time" | bc)
unit_time=$(echo "scale=4; $total_time / $N" | bc)

echo "Total time: $total_time seconds"
echo "Unit time: $unit_time seconds"

occlum stop
