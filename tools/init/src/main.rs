extern crate libc;
extern crate serde;
extern crate serde_json;

use libc::syscall;
use serde::Deserialize;

use std::error::Error;
use std::fs::File;
use std::io::{ErrorKind, Read};

fn main() -> Result<(), Box<dyn Error>> {
    // Load the configuration from initfs
    const IMAGE_CONFIG_FILE: &str = "/etc/image_config.json";
    let image_config = load_config(IMAGE_CONFIG_FILE)?;

    // Get the MAC of Occlum.json.protected file
    let occlum_json_mac = {
        let mut mac: sgx_aes_gcm_128bit_tag_t = Default::default();
        parse_str_to_bytes(&image_config.occlum_json_mac, &mut mac)?;
        mac
    };
    let occlum_json_mac_ptr = &occlum_json_mac as *const sgx_aes_gcm_128bit_tag_t;

    // Get the key of FS image if needed
    let key = match &image_config.image_type[..] {
        "encrypted" => {
            // TODO: Get the key through RA or LA
            const IMAGE_KEY_FILE: &str = "/etc/image_key";
            let key_str = load_key(IMAGE_KEY_FILE)?;
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

    // Mount the image
    const SYS_MOUNT_FS: i64 = 363;
    let ret = unsafe { syscall(SYS_MOUNT_FS, key_ptr, occlum_json_mac_ptr) };
    if ret < 0 {
        return Err(Box::new(std::io::Error::last_os_error()));
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
