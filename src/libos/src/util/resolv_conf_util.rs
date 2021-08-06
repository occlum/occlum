use super::*;
use crate::fs::{AccessMode, CreationFlags, FileMode, FsView};
use resolv_conf::*;
use std::ffi::CStr;
use std::str;

pub fn write_resolv_conf() -> Result<()> {
    const RESOLV_CONF_PATH: &'static str = "/etc/resolv.conf";
    let fs_view = FsView::new();
    // overwrite /etc/resolv.conf if existed
    let resolv_conf_file = fs_view.open_file(
        RESOLV_CONF_PATH,
        AccessMode::O_RDWR as u32 | CreationFlags::O_CREAT.bits() | CreationFlags::O_TRUNC.bits(),
        FileMode::from_bits(0o666).unwrap(),
    )?;
    let resolv_conf_str = RESOLV_CONF_STR.read().unwrap();
    match &*resolv_conf_str {
        Some(str) => {
            resolv_conf_file.write(str.as_bytes());
        }
        None => {}
    }
    Ok(())
}

pub fn parse_resolv_conf(resolv_conf_ptr: *const c_char) -> Result<String> {
    // Read resolv.conf file from host
    let resolv_conf_bytes = unsafe { CStr::from_ptr(resolv_conf_ptr).to_bytes() };
    let resolv_conf_str = str::from_utf8(resolv_conf_bytes)
        .map_err(|_| errno!(EINVAL, "/etc/resolv.conf contains non UTF-8 characters"))?;

    // Parse and inspect resolv.conf file
    if let Err(_) = resolv_conf::Config::parse(resolv_conf_bytes) {
        return_errno!(EINVAL, "malformated host /etc/resolv.conf");
    }
    Ok(resolv_conf_str.to_string())
}
