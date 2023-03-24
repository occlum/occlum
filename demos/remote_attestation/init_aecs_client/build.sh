#! /bin/bash
set -e


UA_ENV_PCCS_URL=${1:-https://sgx-dcap-server.cn-shanghai.aliyuncs.com/sgx/certification/v3/}


function update_client_init_ra_conf()
{
    local ua_pccs_url=$1

    # Fill in the keys
    new_json="$(jq '.kms_keys = [ {"key": "secret-my-keypair", "path": "/etc/saved_secret_rsa_keypair", "service": "service1"}] |
        .kms_keys += [ {"key": "secret-my-aes256-key", "path": "/etc/saved_secret_aes_256", "service": "service1"}]' init_ra_conf.json)" && \
    echo "${new_json}" > init_ra_conf.json

    # Fill in the ua pccs url
    new_json="$(jq .ua_env_pccs_url=\"$ua_pccs_url\" init_ra_conf.json)" && \
    echo "${new_json}" > init_ra_conf.json

    # Fill in the KMS ra_config measurements if necessary.
}

rm -rf occlum_instance
occlum new occlum_instance --init-ra aecs

pushd occlum_instance
rm -rf image
copy_bom -f ../app.yaml --root image --include-dir /opt/occlum/etc/template

new_json="$(jq '.resource_limits.user_space_size = "800MB" |
                .resource_limits.kernel_space_stack_size ="2MB" ' Occlum.json)" && \
    echo "${new_json}" > Occlum.json

# Update init_ra_conf.json
update_client_init_ra_conf $UA_ENV_PCCS_URL

occlum build

popd
