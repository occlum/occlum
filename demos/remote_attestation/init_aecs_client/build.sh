#! /bin/bash
set -e


function update_client_init_ra_conf()
{
    # Fill in the keys
    new_json="$(jq '.kms_keys = [ {"key": "secret-my-keypair", "path": "/etc/saved_secret_rsa_keypair", "service": "service1"}] |
        .kms_keys += [ {"key": "secret-my-aes256-key", "path": "/etc/saved_secret_aes_256", "service": "service1"}]' init_ra_conf.json)" && \
    echo "${new_json}" > init_ra_conf.json

    # Fill in the ua pccs url if for sure
    # otherwise this value could overwritten when running with env UA_ENV_PCCS_URL set

    # Fill in the KMS ra_config measurements if necessary.
}

rm -rf occlum_instance
occlum new occlum_instance --init-ra aecs

pushd occlum_instance
rm -rf image
copy_bom -f ../app.yaml --root image --include-dir /opt/occlum/etc/template

new_json="$(jq '.resource_limits.user_space_size = "800MB" |
                .resource_limits.kernel_space_stack_size ="2MB" |
                .env.untrusted += [ "UA_ENV_PCCS_URL", "OCCLUM_INIT_RA_KMS_SERVER" ]' Occlum.json)" && \
    echo "${new_json}" > Occlum.json

# Update init_ra_conf.json
update_client_init_ra_conf

occlum build

popd
