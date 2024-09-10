#! /bin/bash
set -e

CUR_DIR=$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )
TEST_TIME=${1:-60}
BUF_LEN=${2:-128K}
STREMS=${3:-8}


function iperf3_prepare()
{
    ./build.sh
}

function iperf3_run()
{
    echo ""
    echo "*** Doing iperf3 with ${STREMS} client streams in parallel ***"
    echo "*** with read/write buffer length ${BUF_LEN} for ${TEST_TIME} seconds. ***"

    pushd occlum_server
    occlum run /bin/iperf3 -s -p 6777 -1 1>/dev/null &
    popd

    sleep 3

    pushd occlum_client
    occlum run /bin/iperf3 -c 127.0.0.1 -p 6777 -f Mbits \
        -P ${STREMS} -t ${TEST_TIME} -l ${BUF_LEN} | tee output.txt
    popd
}

function iperf3_iouring_run()
{
    echo ""
    echo "*** Doing iperf3 io_uring with ${STREMS} client streams in parallel ***"
    echo "*** with read/write buffer length ${BUF_LEN} for ${TEST_TIME} seconds. ***"

    pushd occlum_server
    jq '.feature.io_uring = 1' Occlum.json > /tmp.json && mv /tmp.json Occlum.json
    occlum build -f
    occlum run /bin/iperf3 -s -p 6777 -1 1>/dev/null &
    popd

    sleep 3

    pushd occlum_client
    jq '.feature.io_uring = 1' Occlum.json > /tmp.json && mv /tmp.json Occlum.json
    occlum build -f
    occlum run /bin/iperf3 -c 127.0.0.1 -p 6777 -f Mbits \
        -P ${STREMS} -t ${TEST_TIME} -l ${BUF_LEN} | tee output_iouring.txt
    popd
}

function iperf3_result()
{
    output="occlum_client/output.txt"
    SENDER_RES=$(grep "SUM" ${output} | grep "sender" | awk '{print $6}')
    RECV_RES=$(grep "SUM" ${output} | grep "receiver" | awk '{print $6}')

    jq --argjson sender $SENDER_RES --argjson recv $RECV_RES \
        '(.[] | select(.extra == "sender") | .value) |= $sender |
        (.[] | select(.extra == "receiver") | .value) |= $recv'  \
        result_template.json  > result.json

    if [ "$IO_URING_TEST" == "true" ]; then
        output="occlum_client/output_iouring.txt"
        SENDER_RES=$(grep "SUM" ${output} | grep "sender" | awk '{print $6}')
        RECV_RES=$(grep "SUM" ${output} | grep "receiver" | awk '{print $6}')

        jq --argjson sender $SENDER_RES --argjson recv $RECV_RES \
            '(.[] | select(.extra == "sender_iouring") | .value) |= $sender |
            (.[] | select(.extra == "receiver_iouring") | .value) |= $recv'  \
            result.json  > tmp.json

        mv tmp.json result.json
    fi
}

iperf3_prepare
iperf3_run

# Do io_uring mode test
if [ "$IO_URING_TEST" == "true" ]; then
iperf3_iouring_run
fi

iperf3_result
