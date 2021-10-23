//! Built-in ioctls.

use super::*;

pub use self::winsize::*;

mod winsize;

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
