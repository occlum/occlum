extern crate libc;
extern crate serde;
extern crate serde_json;

use libc::syscall;

use std::error::Error;
use std::fs::{write, File};
use std::io::{ErrorKind, Read};
use std::env;

use crate::maa::{maa_generate_json, maa_attestation};
pub mod maa;

fn main() -> Result<(), Box<dyn Error>> {
    // Get the key of FS image if needed
    // If encrypted, get the key through RA or LA
    // key_str format as "a0-73-71-57-2f-c5-3c-dc-ed-2c-51-6a-38-f2-3c-ba"
    // let key = {
    //     // Todo, get the key through RA or LA
    //     let key_str = <get_key_through_ra>;
    //     let mut key: sgx_key_128bit_t = Default::default();
    //     parse_str_to_bytes(&key_str, &mut key)?;
    //     Some(key)
    // };
    // In default, the image is not encrypted
    let key = None;
    let key_ptr = key
        .as_ref()
        .map(|key| key as *const sgx_key_128bit_t)
        .unwrap_or(std::ptr::null());

    // Do Azure attestation and save attestation json to rootfs
    // Get Attestation provider URL, rootfs token path and report data string from env
    let maa_provider_url = env::var("MAA_PROVIDER_URL")
        .unwrap_or("https://shareduks.uks.attest.azure.net".to_string());
    let maa_token_path = env::var("MAA_TOKEN_PATH")
        .unwrap_or("/root".to_string());
    let report_data_base64 = env::var("MAA_REPORT_DATA")
        .unwrap_or("example".to_string());
    let report_data = base64::decode(&report_data_base64).unwrap();

    // Get maa quote json
    let maa_json = maa_generate_json(report_data.as_slice()).unwrap();
    let quote_base64 = serde_json::to_string(&maa_json["quote"]).unwrap();
    // Do maa attestation and get json token response
    let response = maa_attestation(maa_provider_url, maa_json).unwrap();
    let token = serde_json::to_string(&response).unwrap();

    // Mount the image
    const SYS_MOUNT_FS: i64 = 363;
    // User can provide valid rootfs config pointer for runtime mount and boot
    // Otherwise, just pass null pointer to do general mount and boot
    let root_config_ptr: *const i8 = std::ptr::null();
    let ret = unsafe { syscall(
        SYS_MOUNT_FS, key_ptr, root_config_ptr) };
    if ret < 0 {
        return Err(Box::new(std::io::Error::last_os_error()));
    }

    // Write the raw quote and json token to rootfs
    let quote_file = maa_token_path.clone() + "/quote_base64";
    write(quote_file, quote_base64)?;
    let token_file = maa_token_path.clone() + "/token";
    write(token_file, token)?;

    Ok(())
}

#[allow(non_camel_case_types)]
type sgx_key_128bit_t = [u8; 16];

#[allow(dead_code)]
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
