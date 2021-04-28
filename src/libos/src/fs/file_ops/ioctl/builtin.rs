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

type TcflagT = u32;
type CcT = u8;
type SpeedT = u32;
const NCCS: usize = 32;
#[derive(Debug)]
#[repr(C)]
pub struct Termios {
    pub c_iflag: TcflagT,
    pub c_oflag: TcflagT,
    pub c_cflag: TcflagT,
    pub c_lflag: TcflagT,
    pub c_line: CcT,
    pub c_cc: [CcT; NCCS],
    pub c_ispeed: SpeedT,
    pub c_ospeed: SpeedT,
}
