extern crate libc;
extern crate serde;
extern crate serde_json;

use libc::syscall;
use serde::Deserialize;

use std::error::Error;
use std::fs;
use std::fs::File;
use std::io::{ErrorKind, Read};

use std::ffi::CString;
use std::os::raw::{c_int, c_char};

#[link(name = "grpc_ratls_client")]
extern "C" {
    fn grpc_ratls_get_secret(
        server_addr: *const c_char, // grpc server address+port, such as "localhost:50051"
        config_json: *const c_char, // ratls handshake config json file
        name: *const c_char, // secret name to be requested
        secret_file: *const c_char // secret file to be saved
    ) -> c_int;
}

#[link(name = "grpc_ratls_client")]
extern "C" {
    fn grpc_ratls_get_secret_to_buf(
        server_addr: *const c_char, // grpc server address+port, such as "localhost:50051"
        config_json: *const c_char, // ratls handshake config json file
        name: *const c_char, // secret name to be requested
        secret_buf: *const u8, // secret buffer provided by user
        buf_len: *mut u32 // buffer size
    ) -> c_int;
}

fn main() -> Result<(), Box<dyn Error>> {
    // Load the configuration from initfs
    const IMAGE_CONFIG_FILE: &str = "/etc/image_config.json";
    let image_config = load_config(IMAGE_CONFIG_FILE)?;

    // Get client secrets through grpc-ratls
    let server_addr = CString::new("localhost:50051").unwrap();
    let config_json = CString::new("dynamic_config.json").unwrap();

    // Get the key of FS image if needed
    let key = match &image_config.image_type[..] {
        "encrypted" => {
            // Get the image encrypted key through RA
            let secret = CString::new("image_key").unwrap();
            let mut buffer: Vec<u8> = vec![0; 256];
            let buffer_ptr: *const u8 = buffer.as_ptr();
            let mut buffer_len: u32 = buffer.len() as u32;
            let len_ptr: *mut u32 = &mut buffer_len as *mut u32;

            //Read to buffer instead of file system for better security
            let ret = unsafe {
                grpc_ratls_get_secret_to_buf(
                    server_addr.as_ptr(),
                    config_json.as_ptr(),
                    secret.as_ptr(),
                    buffer_ptr,
                    len_ptr
                )
            };

            if ret != 0 {
                println!("grpc_ratls_get_secret failed return {}", ret);
                return Err(Box::new(std::io::Error::last_os_error()));
            }

            buffer.resize(buffer_len as usize, 0);
            let key_string = String::from_utf8(buffer)
                .expect("error converting to string");
            let key_str = key_string
                .trim_end_matches(|c| c == '\r' || c == '\n').to_string();
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

    // Get certificate
    let secret = CString::new("flask_cert").unwrap();
    let filename = CString::new("cert_file").unwrap();

    let ret = unsafe {
        grpc_ratls_get_secret(
            server_addr.as_ptr(),
            config_json.as_ptr(),
            secret.as_ptr(),
            filename.as_ptr())
    };

    if ret != 0 {
        println!("grpc_ratls_get_secret failed return {}", ret);
        return Err(Box::new(std::io::Error::last_os_error()));
    }

    let cert_secret = fs::read_to_string(filename.into_string().unwrap())
        .expect("Something went wrong reading the file");

    // Get key
    let secret = CString::new("flask_key").unwrap();
    let filename = CString::new("key_file").unwrap();

    let ret = unsafe {
        grpc_ratls_get_secret(
            server_addr.as_ptr(),
            config_json.as_ptr(),
            secret.as_ptr(),
            filename.as_ptr())
    };

    if ret != 0 {
        println!("grpc_ratls_get_secret failed return {}", ret);
        return Err(Box::new(std::io::Error::last_os_error()));
    }

    let key_secret = fs::read_to_string(filename.into_string().unwrap())
        .expect("Something went wrong reading the file");

    // Mount the image
    const SYS_MOUNT_FS: i64 = 363;
    // User can provide valid path for runtime mount and boot
    // Otherwise, just pass null pointer to do general mount and boot
    let root_config_path: *const i8 = std::ptr::null();
    let ret = unsafe { syscall(SYS_MOUNT_FS, key_ptr, root_config_path) };
    if ret < 0 {
        return Err(Box::new(std::io::Error::last_os_error()));
    }

    // Write the secrets to rootfs
    fs::write("/etc/flask.crt", cert_secret.into_bytes())?;
    fs::write("/etc/flask.key", key_secret.into_bytes())?;

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
