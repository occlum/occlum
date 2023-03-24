extern crate libc;
extern crate serde;
extern crate serde_json;

use libc::syscall;
use serde::{Deserialize, Serialize};

use std::error::Error;
use std::fs;
use std::fs::File;
use std::io::{ErrorKind, Read};
use std::str;
use std::env;

use std::ffi::CString;
use std::os::raw::{c_int, c_char};

#[link(name = "aecs_client")]
extern "C" {
    fn aecs_client_get_secret_and_save_file(
        aec_server_endpoint: *const c_char,
        aecs_server_policy: *const c_char,
        secret_service: *const c_char,
        secret_name: *const c_char,
        nonce: *const c_char,
        save_file_name: *const c_char
    ) -> c_int;
}

#[derive(Deserialize, Serialize, Debug)]
#[warn(dead_code)]
struct MRsValue {
    pub mr_enclave: String,
    pub mr_signer: String,
    pub isv_prod_id: u32,
    pub isv_svn: u32,
    pub config_svn: u32,
    pub debuggable: bool,
}

#[derive(Deserialize, Serialize, Debug)]
#[warn(dead_code)]
struct RAConfig {
    ua_ias_url: String,
    ua_ias_spid: String,
    ua_ias_apk_key: String,
    ua_dcap_lib_path: String,
    ua_dcap_pccs_url: String,
    ua_uas_url: String,
    ua_uas_app_key: String,
    ua_uas_app_secret: String,
    ua_policy_str_tee_platform: String,
    ua_policy_hex_platform_hw_version: String,
    ua_policy_hex_platform_sw_version: String,
    ua_policy_hex_secure_flags: String,
    ua_policy_hex_platform_measurement: String,
    ua_policy_hex_boot_measurement: String,
    ua_policy_str_tee_identity: String,
    ua_policy_hex_ta_measurement: String,
    ua_policy_hex_ta_dyn_measurement: String,
    ua_policy_hex_signer: String,
    ua_policy_hex_prod_id: String,
    ua_policy_str_min_isvsvn: String,
    ua_policy_hex_user_data: String,
    ua_policy_bool_debug_disabled: String,
    ua_policy_hex_hash_or_pem_pubkey: String,
    ua_policy_hex_nonce: String,
    ua_policy_hex_spid: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[warn(dead_code)]
struct KmsKeys {
    key: String,
    path: String,
    service: String,
}

#[derive(Deserialize, Serialize, Debug)]
#[warn(dead_code)]
struct InitRAConfig {
    kms_server: String,
    kms_keys: Vec<KmsKeys>,
    ua_env_pccs_url: String,
    ra_config: RAConfig
}

fn load_ra_config(ra_conf_path: &str) -> Result<InitRAConfig, Box<dyn Error>> {
    let mut ra_conf_file = File::open(ra_conf_path)?;
    let ra_conf = {
        let mut ra_conf = String::new();
        ra_conf_file.read_to_string(&mut ra_conf)?;
        ra_conf
    };
    let config: InitRAConfig = serde_json::from_str(&ra_conf)?;
    Ok(config)
}


fn main() -> Result<(), Box<dyn Error>> {
    // Load the configuration from initfs
    const IMAGE_CONFIG_FILE: &str = "/etc/image_config.json";
    const INIT_RA_CONF: &str = "/etc/init_ra_conf.json";
    let image_config = load_config(IMAGE_CONFIG_FILE)?;

    // Get the MAC of Occlum.json.protected file
    let occlum_json_mac = {
        let mut mac: sgx_aes_gcm_128bit_tag_t = Default::default();
        parse_str_to_bytes(&image_config.occlum_json_mac, &mut mac)?;
        mac
    };
    let occlum_json_mac_ptr = &occlum_json_mac as *const sgx_aes_gcm_128bit_tag_t;

    // Do parse to get Init RA information
    let init_ra_conf = load_ra_config(INIT_RA_CONF)?;
    // Extract RA config part
    let ra_conf_string = serde_json::to_string_pretty(&init_ra_conf.ra_config).unwrap();
    fs::create_dir_all("/etc/kubetee")?;
    fs::write("/etc/kubetee/unified_attestation.json", ra_conf_string.clone().into_bytes())?;

    let server_addr = CString::new(init_ra_conf.kms_server).unwrap();
    env::set_var("UA_ENV_PCCS_URL", init_ra_conf.ua_env_pccs_url.clone());

    // Get the key of FS image if needed
    let key = match &image_config.image_type[..] {
        "encrypted" => {
            // Get the image encrypted key through RA
            let secret = CString::new("image_key").unwrap();
            let service = CString::new("service1").unwrap();
            let filename = CString::new("/etc/image_key").unwrap();

            let ret = unsafe {
                aecs_client_get_secret_and_save_file(
                    server_addr.as_ptr(),
                    std::ptr::null(),
                    service.as_ptr(),
                    secret.as_ptr(),
                    std::ptr::null(),
                    filename.as_ptr())
            };

            if ret != 0 {
                println!("grpc_ratls_get_secret failed return {}", ret);
                return Err(Box::new(std::io::Error::last_os_error()));
            }

            const IMAGE_KEY_FILE: &str = "/etc/image_key";
            let key_str = load_key(IMAGE_KEY_FILE)?;
            // Remove key file which is not needed any more
            fs::remove_file(IMAGE_KEY_FILE)?;
            let mut key: sgx_key_128bit_t = Default::default();
            parse_str_to_bytes(&key_str, &mut key)?;
            Some(key)
        }
        "integrity-only" => None,
        _ => unreachable!(),
    };
    let key_ptr = key
        .as_ref()
        .map(|key| key as *const sgx_key_128bit_t)
        .unwrap_or(std::ptr::null());

    // Do one time key acquire to force all necessary libraries got loaded to memory.
    // Thus after mount rootfs, the API could run successfully.
    // The key got this time will be dropped.
    unsafe {
        let kms_key = init_ra_conf.kms_keys[0].clone();
        let key = CString::new(kms_key.key).unwrap();
        let file = CString::new(kms_key.path).unwrap();
        let service =CString::new(kms_key.service).unwrap();

        aecs_client_get_secret_and_save_file(
            server_addr.as_ptr(),
            std::ptr::null(),
            service.as_ptr(),
            key.as_ptr(),
            std::ptr::null(),
            file.as_ptr());
        // Remove key file which is not needed any more
        fs::remove_file(file.into_string().unwrap())?;
    };

    // Mount the image
    const SYS_MOUNT_FS: i64 = 363;
    let ret = unsafe { syscall(SYS_MOUNT_FS, key_ptr, occlum_json_mac_ptr) };
    if ret < 0 {
        return Err(Box::new(std::io::Error::last_os_error()));
    }

    // Rewrite ra_config to rootfs
    fs::create_dir_all("/etc/kubetee")?;
    fs::write("/etc/kubetee/unified_attestation.json", ra_conf_string.clone().into_bytes())?;

    // Get keys and save to path
    for keys in init_ra_conf.kms_keys {
        let key = CString::new(keys.key).unwrap();
        let file = CString::new(keys.path).unwrap();
        let service =CString::new(keys.service).unwrap();

        let ret = unsafe {
            aecs_client_get_secret_and_save_file(
                server_addr.as_ptr(),
                std::ptr::null(),
                service.as_ptr(),
                key.as_ptr(),
                std::ptr::null(),
                file.as_ptr())
        };

        if ret != 0 {
            println!("Failed to get key {:?}, return {}", key, ret);
            // return Err(Box::new(std::io::Error::last_os_error()));
        }
    }

    Ok(())
}

#[allow(non_camel_case_types)]
type sgx_key_128bit_t = [u8; 16];
#[allow(non_camel_case_types)]
type sgx_aes_gcm_128bit_tag_t = [u8; 16];

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
struct ImageConfig {
    occlum_json_mac: String,
    image_type: String,
}

fn load_config(config_path: &str) -> Result<ImageConfig, Box<dyn Error>> {
    let mut config_file = File::open(config_path)?;
    let config_json = {
        let mut config_json = String::new();
        config_file.read_to_string(&mut config_json)?;
        config_json
    };
    let config: ImageConfig = serde_json::from_str(&config_json)?;
    Ok(config)
}

fn load_key(key_path: &str) -> Result<String, Box<dyn Error>> {
    let mut key_file = File::open(key_path)?;
    let mut key = String::new();
    key_file.read_to_string(&mut key)?;
    Ok(key.trim_end_matches(|c| c == '\r' || c == '\n').to_string())
}

fn parse_str_to_bytes(arg_str: &str, bytes: &mut [u8]) -> Result<(), Box<dyn Error>> {
    let bytes_str_vec = {
        let bytes_str_vec: Vec<&str> = arg_str.split('-').collect();
        if bytes_str_vec.len() != bytes.len() {
            return Err(Box::new(std::io::Error::new(
                ErrorKind::InvalidData,
                "The length or format of Key/MAC string is invalid",
            )));
        }
        bytes_str_vec
    };

    for (byte_i, byte_str) in bytes_str_vec.iter().enumerate() {
        bytes[byte_i] = u8::from_str_radix(byte_str, 16)?;
    }
    Ok(())
}
