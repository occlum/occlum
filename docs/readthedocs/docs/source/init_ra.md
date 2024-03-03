# Init RA Solutions

There are two **Init-RA** solutions provided, **GRPC-RATLS** and **AECS**.

With these two solutions, two customized **init** are provided. Thus users don't need modify the **init** by themselves. Users only need fill in some fields in the template `init_ra_conf.json`

## AECS Init-RA

[AECS](https://github.com/SOFAEnclave/enclave-configuration-service) is a short name of **Attestation based Enclave Configuration Service**. Basically, part of its function is acting as a remote attestation based key management service. 

Occlum provides a way to embed the AECS client function in Occlum Init process by simply running `occlum new occlum_instance --init-ra aecs` to initiate an Occlum instance.

Then, users can modify the template `init_ra_conf.json` in oclcum_instance accordingly.

```json
{
    "kms_server": "localhost:19527",
    "kms_keys": [
        {
            "key": "demo_key",
            "path": "/etc/demo_key",
            "service": "service"
        }
    ],
    "ua_env_pccs_url": "",
    "ra_config": {
        "ua_ias_url": "https://api.trustedservices.intel.com/sgx/dev/attestation/v4",
        "ua_ias_spid": "",
        "ua_ias_apk_key": "",
        "ua_dcap_lib_path": "",
        "ua_dcap_pccs_url": "",
        "ua_uas_url": "",
        "ua_uas_app_key": "",
        "ua_uas_app_secret": "",
        "ua_policy_str_tee_platform": "",
        "ua_policy_hex_platform_hw_version": "",
        "ua_policy_hex_platform_sw_version": "",
        "ua_policy_hex_secure_flags": "",
        "ua_policy_hex_platform_measurement": "",
        "ua_policy_hex_boot_measurement": "",
        "ua_policy_str_tee_identity": "",
        "ua_policy_hex_ta_measurement": "",
        "ua_policy_hex_ta_dyn_measurement": "",
        "ua_policy_hex_signer": "",
        "ua_policy_hex_prod_id": "",
        "ua_policy_str_min_isvsvn": "",
        "ua_policy_hex_user_data": "",
        "ua_policy_bool_debug_disabled": "",
        "ua_policy_hex_hash_or_pem_pubkey": "",
        "ua_policy_hex_nonce": "",
        "ua_policy_hex_spid": ""
    }
}
```

The json file specifies:

* The URL of the KMS server (AECS).

**"kms_server"** in the json file, which can be overwritten by environment value **OCCLUM_INIT_RA_KMS_SERVER** when running.

* The secrets users need acquire and where to put.

**"kms_keys"** part. It can define multiple keys to be acquired from KMS server (AECS), and the paths to save the keys. This part should align with the keys injected into KMS server (AECS).

* The PCCS URL.

**"ua_env_pccs_url"**. It should be the same with the `"pccs_url"` in the file `/etc/sgx_default_qcnl.conf`. It also could be overwritten by environment value **UA_ENV_PCCS_URL** when running.

* The measurement of the KMS server (AECS) to be trusted.

**"ra_config"** part defines the information of the KMS server (AECS) to be trusted. Users could ignore this part if **"kms_server"** is guaranted to be trusted. Otherwise, some fields, ususally **ua_policy_*** measurements, are expected correspoding values -- RA measurement values from the correct KMS server (AECS).


There is a demo [init_aecs_client](https://github.com/occlum/occlum/tree/master/demos/remote_attestation/init_aecs_client) for reference.

## GRPC-RATLS Init-RA

It is based on a GRPC-RATLS implementation.

Occlum provides a way to embed the AECS client function in Occlum Init process by simply running `occlum new occlum_instance --init-ra grpc_ratls` to initiate an Occlum instance.

Then, users can modify the template `init_ra_conf.json` in oclcum_instance accordingly.

```json
{
    "kms_server": "localhost:50051",
    "kms_keys": [
        {
            "key": "demo_key",
            "path": "/etc/demo_key"
        }
    ],
    "ra_config": {
        "verify_mr_enclave" : "off",
        "verify_mr_signer" : "off",
        "verify_isv_prod_id" : "off",
        "verify_isv_svn" : "off",
        "verify_config_svn" : "off",
        "verify_enclave_debuggable" : "off",
        "sgx_mrs": [
            {
                "mr_enclave" : "",
                "mr_signer" : "",
                "isv_prod_id" : 0,
                "isv_svn" : 0,
                "config_svn" : 0,
                "debuggable" : true
            }
        ]
    }
}
```
The json file specifies:

* The URL of the KMS server (GRPC RA Server).

**"kms_server"** in the json file, which can be overwritten by environment value **OCCLUM_INIT_RA_KMS_SERVER** when running.

* The secrets users need acquire and where to put.

**"kms_keys"** part. It can define multiple keys to be acquired from KMS server (GRPC RA Server), and the paths to save the keys. This part should align with the keys injected into KMS server (GRPC RA Server).

* The measurement of the KMS server (GRPC RA Server) to be trusted.

**"ra_config"** part defines the information of the KMS server (GRPC RA Server) to be trusted. Users could ignore this part if **"kms_server"** is guaranted to be trusted. Otherwise, some fields are expected correspoding values -- RA measurement values from the correct KMS server (GRPC RA Server).


Details please refer to the demo [init_ra_flow](https://github.com/occlum/occlum/tree/master/demos/remote_attestation/init_ra_flow).
