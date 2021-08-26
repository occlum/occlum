use super::*;
use crate::fs::{AccessMode, CreationFlags, FileMode, FsView};
use resolv_conf::*;
use std::ffi::CStr;
use std::str;

#[repr(C)]
pub struct host_file_buffer {
    pub resolv_conf_ptr: *const c_char,
    pub hosts_ptr: *const c_char,
    pub hostname_ptr: *const c_char,
}

pub enum HostFile {
    HOSTS,
    HOSTNAME,
    RESOLV_CONF,
}

pub fn write_host_file(host_file: HostFile) -> Result<()> {
    let file_path: &str = match host_file {
        HostFile::HOSTS => "/etc/hosts",
        HostFile::HOSTNAME => "/etc/hostname",
        HostFile::RESOLV_CONF => "/etc/resolv.conf",
        _ => return_errno!(EINVAL, "Unsupported host file"),
    };

    let fs_view = FsView::new();
    // overwrite host file if existed in Occlum fs
    let enclave_file = fs_view.open_file(
        file_path,
        AccessMode::O_RDWR as u32 | CreationFlags::O_CREAT.bits() | CreationFlags::O_TRUNC.bits(),
        FileMode::from_bits(0o666).unwrap(),
    )?;

    let host_file_str = match host_file {
        HostFile::HOSTS => HOSTS_STR.read().unwrap(),
        HostFile::HOSTNAME => HOSTNAME_STR.read().unwrap(),
        HostFile::RESOLV_CONF => RESOLV_CONF_STR.read().unwrap(),
        _ => return_errno!(EINVAL, "Unsupported host file"),
    };

    match &*host_file_str {
        Some(str) => {
            enclave_file.write(str.as_bytes());
        }
        None => {
            warn!("The host file: {:?} does not exist", file_path);
        }
    }
    Ok(())
}

pub fn parse_host_file(host_file: HostFile, host_file_ptr: *const c_char) -> Result<String> {
    // Read host file
    let host_file_bytes = unsafe { CStr::from_ptr(host_file_ptr).to_bytes() };
    let host_file_str = str::from_utf8(host_file_bytes)
        .map_err(|_| errno!(EINVAL, "host file contains non UTF-8 characters"))?;

    match host_file {
        HostFile::HOSTS => {
            // TODO: Parsing hosts
        }
        HostFile::HOSTNAME => {
            // TODO: Parsing hostname
        }
        HostFile::RESOLV_CONF => {
            // Parse and inspect host file
            if let Err(_) = resolv_conf::Config::parse(host_file_bytes) {
                return_errno!(EINVAL, "malformated host /etc/resolv.conf");
            }
        }
        _ => return_errno!(EINVAL, "Unsupported host file"),
    };

    Ok(host_file_str.to_string())
}
