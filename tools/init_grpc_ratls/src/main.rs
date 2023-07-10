extern crate libc;
extern crate serde;
extern crate serde_json;

use libc::syscall;
use serde::{Deserialize, Serialize};

use std::env;
use std::error::Error;
use std::fs;
use std::fs::File;
use std::io::{ErrorKind, Read};
use std::str;

use std::ffi::CString;
use std::os::raw::{c_char, c_int};

#[link(name = "grpc_ratls_client")]
extern "C" {
    fn grpc_ratls_get_secret_to_buf(
        server_addr: *const c_char, // grpc server address+port, such as "localhost:50051"
        config_json: *const c_char, // ratls handshake config json file
        name: *const c_char,        // secret name to be requested
        secret_buf: *const u8,      // secret buffer provided by user
        buf_len: *mut u32,          // buffer size
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
    verify_mr_enclave: String,
    verify_mr_signer: String,
    verify_isv_prod_id: String,
    verify_isv_svn: String,
    verify_config_svn: String,
    verify_enclave_debuggable: String,
    sgx_mrs: Vec<MRsValue>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[warn(dead_code)]
struct KmsKeys {
    key: String,
    path: String,
}

#[derive(Deserialize, Serialize, Debug)]
#[warn(dead_code)]
struct InitRAConfig {
    kms_server: String,
    kms_keys: Vec<KmsKeys>,
    ra_config: RAConfig,
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

struct KeyInfo {
    path: String,
    val_buf: Vec<u8>,
}

fn get_kms_keys(
    kms_keys: Vec<KmsKeys>,
    kms_server: CString,
    kms_config: CString,
) -> Result<Vec<KeyInfo>, Box<dyn Error>> {
    let mut keys_info: Vec<KeyInfo> = Vec::new();
    for keys in kms_keys {
        let key = CString::new(&*keys.key).unwrap();
        // Max key length is 10K
        let mut buffer: Vec<u8> = vec![0; 10240];
        let mut buffer_len: u32 = buffer.len() as u32;

        let ret = unsafe {
            grpc_ratls_get_secret_to_buf(
                kms_server.as_ptr(),
                kms_config.as_ptr(),
                key.as_ptr(),
                buffer.as_ptr(),
                &mut buffer_len,
            )
        };

        if ret != 0 {
            let err_msg = format!("grpc_ratls client get secret error: {}", ret);
            return Err(Box::new(std::io::Error::new(ErrorKind::Other, err_msg)));
        }

        buffer.resize(buffer_len as usize, 0);

        let key_info: KeyInfo = KeyInfo {
            path: keys.path.clone(),
            val_buf: buffer.clone(),
        };

        keys_info.push(key_info);
    }
    Ok(keys_info)
}

fn main() -> Result<(), Box<dyn Error>> {
    // Load the configuration from initfs
    const IMAGE_CONFIG_FILE: &str = "/etc/image_config.json";
    const INIT_RA_CONF: &str = "/etc/init_ra_conf.json";
    let image_config = load_config(IMAGE_CONFIG_FILE)?;

    // Do parse to get Init RA information
    let init_ra_conf = load_ra_config(INIT_RA_CONF)?;
    // Extract RA config part
    let ra_conf_string = serde_json::to_string_pretty(&init_ra_conf.ra_config).unwrap();
    fs::write("ra_config.json", ra_conf_string.clone().into_bytes())?;
    let config_json = CString::new("ra_config.json").unwrap();

    // grpc server address from environment has higher priority
    let server_addr =
        CString::new(env::var("OCCLUM_INIT_RA_KMS_SERVER").unwrap_or(init_ra_conf.kms_server))
            .unwrap();

    // Get the key of FS image if needed
    let key = match &image_config.image_type[..] {
        "encrypted" => {
            // Get the image encrypted key through RA
            let secret = CString::new("image_key").unwrap();
            let mut buffer: Vec<u8> = vec![0; 256];
            let mut buffer_len: u32 = buffer.len() as u32;

            //Read to buffer instead of file system for better security
            let ret = unsafe {
                grpc_ratls_get_secret_to_buf(
                    server_addr.as_ptr(),
                    config_json.as_ptr(),
                    secret.as_ptr(),
                    buffer.as_ptr(),
                    &mut buffer_len,
                )
            };

            if ret != 0 {
                let err_msg = format!("grpc_ratls client get secret error: {}", ret);
                return Err(Box::new(std::io::Error::new(ErrorKind::Other, err_msg)));
            }

            buffer.resize(buffer_len as usize, 0);
            let key_string = String::from_utf8(buffer).expect("error converting to string");
            let key_str = key_string
                .trim_end_matches(|c| c == '\r' || c == '\n')
                .to_string();
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

    // Get keys from kms if any
    let keys_info: Vec<KeyInfo> = get_kms_keys(init_ra_conf.kms_keys, server_addr, config_json)?;
    // Remove config file
    fs::remove_file("ra_config.json")?;

    // Mount the image
    const SYS_MOUNT_FS: i64 = 363;
    // User can provide valid path for runtime mount and boot
    // Otherwise, just pass null pointer to do general mount and boot
    let root_config_path: *const i8 = std::ptr::null();
    let ret = unsafe { syscall(SYS_MOUNT_FS, key_ptr, root_config_path) };
    if ret < 0 {
        return Err(Box::new(std::io::Error::last_os_error()));
    }

    // Get keys and save to path
    for key_info in keys_info {
        fs::write(key_info.path, key_info.val_buf.as_slice())?;
    }

    Ok(())
}

#[allow(non_camel_case_types)]
type sgx_key_128bit_t = [u8; 16];

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
struct ImageConfig {
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
