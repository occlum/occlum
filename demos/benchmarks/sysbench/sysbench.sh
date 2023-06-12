#! /bin/bash
set -e

CUR_DIR=$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )
TEST_TIME=${1:-60}
TEST_THREADS=${2:-200}

function sysbench_prepare()
{
    ./dl_and_build.sh
    ./prepare_sysbench.sh
}

function sysbench_run()
{
    echo ""
    echo "*** Doing sysbench with ${TEST_THREADS} threads for ${TEST_TIME} seconds ***"

    pushd occlum_instance
    occlum run /bin/sysbench threads \
        --threads=${TEST_THREADS} --thread-yields=100 \
        --thread-locks=4 --time=${TEST_TIME} run | tee output.txt
    popd
}

function sysbench_result()
{
    output="occlum_instance/output.txt"
    MIN=$(grep "min:" ${output} | awk '{print $NF}')
    AVG=$(grep "avg:" ${output} | awk '{print $NF}')
    MAX=$(grep "max:" ${output} | awk '{print $NF}')
    PER95=$(grep "95th" ${output} | awk '{print $NF}')

    jq --argjson min $MIN --argjson avg $AVG --argjson max $MAX --argjson per95 $PER95 \
        '(.[] | select(.extra == "min") | .value) |= $min |
        (.[] | select(.extra == "avg") | .value) |= $avg |
        (.[] | select(.extra == "max") | .value) |= $max |
        (.[] | select(.extra == "per95") | .value) |= $per95'  \
        result_template.json  > result.json
}

sysbench_prepare
sysbench_run
sysbench_result
