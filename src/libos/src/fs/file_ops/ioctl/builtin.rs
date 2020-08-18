//! Built-in ioctls.

use super::*;

#[derive(Debug)]
#[repr(C)]
pub struct WinSize {
    pub ws_row: u16,
    pub ws_col: u16,
    pub ws_xpixel: u16,
    pub ws_ypixel: u16,
}

#[derive(Debug)]
#[repr(C)]
pub struct IfConf {
    pub ifc_len: i32,
    pub ifc_buf: *const u8,
}

const IFNAMSIZ: usize = 16;
#[derive(Debug)]
#[repr(C)]
pub struct IfReq {
    pub ifr_name: [u8; IFNAMSIZ],
    pub ifr_union: [u8; 24],
}
