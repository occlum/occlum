use super::*;
use crate::fs::{AccessMode, CreationFlags, FileMode, FsView};
use resolv_conf::*;
use std::ffi::CStr;
use std::str;

#[repr(C)]
pub struct host_file_buffer {
    pub resolv_conf_buf: *const c_char,
    resolv_conf_buf_size: u32,
    pub hosts_buf: *const c_char,
    hosts_buf_size: u32,
    pub hostname_buf: *const c_char,
    hostname_buf_size: u32,
}

pub enum HostFile {
    Hosts,
    HostName,
    ResolvConf,
}

pub fn write_host_file(host_file: HostFile) -> Result<()> {
    let file_path: &str = match host_file {
        HostFile::Hosts => "/etc/hosts",
        HostFile::HostName => "/etc/hostname",
        HostFile::ResolvConf => "/etc/resolv.conf",
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
        HostFile::Hosts => HOSTS_STR.read().unwrap(),
        HostFile::HostName => HOSTNAME_STR.read().unwrap(),
        HostFile::ResolvConf => RESOLV_CONF_STR.read().unwrap(),
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

    // Parse and inspect host file
    match host_file {
        HostFile::Hosts => {
            if let Err(_) = hosts_parser_util::parse_hosts_buffer(host_file_bytes) {
                return_errno!(EINVAL, "malformated host /etc/hosts");
            }
        }
        HostFile::HostName => match hosts_parser_util::parse_hostname_buffer(host_file_bytes) {
            Err(_) => {
                return_errno!(EINVAL, "malformated host /etc/hostname");
            }
            Ok(hostname_str) => {
                return Ok(hostname_str);
            }
        },
        HostFile::ResolvConf => {
            if let Err(_) = resolv_conf::Config::parse(host_file_bytes) {
                return_errno!(EINVAL, "malformated host /etc/resolv.conf");
            }
        }
        _ => return_errno!(EINVAL, "Unsupported host file"),
    };

    Ok(host_file_str.to_string())
}
