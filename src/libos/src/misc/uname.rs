use super::*;
use std::ffi::{CStr, CString};
/// A sample of `struct utsname`
/// ```
///   sysname = Linux
///   nodename = tian-nuc
///   release = 4.15.0-42-generic
///   version = #45~16.04.1-Ubuntu SMP Mon Nov 19 13:02:27 UTC 2018
///   machine = x86_64
///   domainname = (none)
/// ```
///
/// By the way, UTS stands for UNIX Timesharing System.
#[repr(C)]
#[derive(Copy, Clone)]
pub struct utsname_t {
    sysname: [u8; 65],
    nodename: [u8; 65],
    release: [u8; 65],
    version: [u8; 65],
    machine: [u8; 65],
    domainname: [u8; 65],
}

pub fn do_uname(name: &mut utsname_t) -> Result<()> {
    copy_from_cstr_to_u8_array(&SYSNAME, &mut name.sysname);
    copy_from_cstr_to_u8_array(&NODENAME.read().unwrap(), &mut name.nodename);
    copy_from_cstr_to_u8_array(&RELEASE, &mut name.release);
    copy_from_cstr_to_u8_array(&VERSION, &mut name.version);
    copy_from_cstr_to_u8_array(&MACHINE, &mut name.machine);
    copy_from_cstr_to_u8_array(&DOMAINNAME, &mut name.domainname);
    Ok(())
}

lazy_static! {
    static ref SYSNAME: CString = CString::new("Occlum").unwrap();
    static ref NODENAME: RwLock<CString> = RwLock::new(CString::new("occlum-node").unwrap());
    static ref RELEASE: CString = CString::new("0.1").unwrap();
    static ref VERSION: CString = CString::new("0.1").unwrap();
    static ref MACHINE: CString = CString::new("x86-64").unwrap();
    static ref DOMAINNAME: CString = CString::new("").unwrap();
}

fn copy_from_cstr_to_u8_array(src: &CStr, dst: &mut [u8]) {
    let src: &[u8] = src.to_bytes_with_nul();
    let len = min(dst.len() - 1, src.len());
    dst[..len].copy_from_slice(&src[..len]);
    dst[len] = 0;
}

pub fn init_nodename(nodename_str: &str) {
    let nodename_cstr = CString::new(nodename_str).unwrap();
    let mut nodename = NODENAME.write().unwrap();
    *nodename = nodename_cstr;
}
