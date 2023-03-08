#! /bin/bash
set -e

CUR_DIR=$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )
MICRO_CONFIG="fio-microbench.fio"
MICRO_PATH=$1

function fio_prepare()
{
    if [ ! -d fio_src ];then
        echo -e "Download and build FIO first"
        ./download_and_build_fio.sh
    fi
}

function fio_run()
{
    echo ""
    echo "*** Doing FIO microbenchmarks ***"

    ./run_fio_on_occlum.sh ${MICRO_CONFIG} ${MICRO_PATH} | tee output.txt
}

function fio_result()
{
    output="${CUR_DIR}/output.txt"

    # Parse write-test results
    write_output=$(grep "WRITE:" ${output} | awk '{print $2}')
    write_seq=$(echo ${write_output} | awk '{print $1}')
    WRITE_SEQ=${write_seq:3:-5}
    write_rand=$(echo ${write_output} | awk '{print $2}')
    WRITE_RAND=${write_rand:3:-5}

    # Parse read-test results
    read_output=$(grep "READ:" ${output} | awk '{print $2}')
    read_seq=$(echo ${read_output} | awk '{print $1}')
    READ_SEQ=${read_seq:3:-5}
    read_rand=$(echo ${read_output} | awk '{print $2}')
    READ_RAND=${read_rand:3:-5}

    jq --argjson seqwrite $WRITE_SEQ --argjson randwrite $WRITE_RAND --argjson seqread $READ_SEQ --argjson randread $READ_RAND \
        '(.[] | select(.extra == "seqwrite") | .value) |= $seqwrite |
        (.[] | select(.extra == "randwrite") | .value) |= $randwrite |
        (.[] | select(.extra == "seqread") | .value) |= $seqread |
        (.[] | select(.extra == "randread") | .value) |= $randread'  \
        result_template.json  > result.json

}

fio_prepare
fio_run
fio_result
