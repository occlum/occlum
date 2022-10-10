#!/bin/bash
set -e

BLUE='\033[1;34m'
NC='\033[0m'
INSTANCE_DIR="occlum_instance"
bomfile="../maa.yaml"
json_dir=$PWD/out

# parameter 1 defines the predefined Occlum json
# parameter 2 defines the generated maa json
function gen_maa_json() {
    config=$1
    maa=${2:-"maa.json"}

    if [ ! -f $config ]; then
        echo "Please provide valid Occlum json file"
        exit -1
    fi

    rm -rf ${INSTANCE_DIR} && occlum new ${INSTANCE_DIR}
    pushd ${INSTANCE_DIR}

    rm -rf image
    copy_bom -f $bomfile --root image --include-dir /opt/occlum/etc/template
    cp ../$config Occlum.json
    occlum build

    echo -e "${BLUE}occlum run to generate quote in maa json format${NC}"
    occlum run /bin/gen_maa_json

    echo -e "${BLUE}Generated maa json file ${json_dir}/$maa${NC}"
    mv maa.json ${json_dir}/$maa
    popd
}

echo "*** Build glibc maa demo ***"
make -C gen_quote clean
make -C gen_quote

rm -rf ${json_dir} && mkdir -p ${json_dir}

gen_maa_json ./config/Occlum.json maa_debug.json
gen_maa_json ./config/Occlum_prodid.json maa_prodid.json
gen_maa_json ./config/Occlum_release.json maa_release.json
gen_maa_json ./config/Occlum_securityversion.json maa_securityversion.json


