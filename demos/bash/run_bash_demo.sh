#! /bin/bash
set -e

if [[ $1 == "musl" ]]; then
    echo "*** Run musl-libc bash demo ***"
    ./prepare_busybox.sh musl
    bomfile="../bash-musl.yaml"
else
    echo "*** Run glibc bash demo ***"
    ./prepare_busybox.sh
    bomfile="../bash.yaml"
fi

rm -rf occlum_instance
occlum new occlum_instance

pushd occlum_instance
rm -rf image
copy_bom -f $bomfile --root image --include-dir /opt/occlum/etc/template

new_json="$(jq '.resource_limits.user_space_size = "600MB" ' Occlum.json)" && \
    echo "${new_json}" > Occlum.json

occlum build
occlum run /bin/occlum_bash_test.sh

popd
