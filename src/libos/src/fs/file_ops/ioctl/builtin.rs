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

/*
 The termios structure used in the Linux kernel is not the same as we use in the glibc. Thus, we have two
 definitions here.
*/

const KERNEL_NCCS: usize = 19;
const NCCS: usize = 32;

type TcflagT = u32;
type CcT = u8;
type SpeedT = u32;

// Corresponds to the definition in glibc: sysdeps/unix/sysv/linux/kernel_termios.h
#[derive(Debug, Default, Copy, Clone)]
#[repr(C)]
pub struct KernelTermios {
    pub c_iflag: TcflagT,
    pub c_oflag: TcflagT,
    pub c_cflag: TcflagT,
    pub c_lflag: TcflagT,
    pub c_line: CcT,
    pub c_cc: [CcT; KERNEL_NCCS],
}

#[derive(Debug, Default, Copy, Clone)]
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

impl KernelTermios {
    fn to_termios(&self) -> Termios {
        let mut c_cc = [0; NCCS];
        c_cc[..KERNEL_NCCS].copy_from_slice(&self.c_cc);
        Termios {
            c_iflag: self.c_iflag,
            c_oflag: self.c_oflag,
            c_cflag: self.c_cflag,
            c_lflag: self.c_lflag,
            c_line: self.c_line,
            c_cc: c_cc,
            c_ispeed: 0,
            c_ospeed: 0,
        }
    }

    pub fn execute_tcgets(&mut self, host_fd: i32, cmd_num: i32) -> Result<i32> {
        debug_assert!(cmd_num == 0x5401);
        let mut termios = self.to_termios();
        let len = std::mem::size_of::<Termios>();
        let ret = try_libc!({
            let mut retval: i32 = 0;
            let status = occlum_ocall_ioctl(
                &mut retval as *mut i32,
                host_fd,
                cmd_num,
                &mut termios as *const Termios as *mut c_void,
                len,
            );
            assert!(status == sgx_status_t::SGX_SUCCESS);
            retval
        });
        *self = termios.to_kernel_termios();
        Ok(ret)
    }

    pub fn execute_tcsets(&self, host_fd: i32, cmd_num: i32) -> Result<i32> {
        debug_assert!(cmd_num == 0x5402);
        let termios = self.to_termios();
        let len = std::mem::size_of::<Termios>();
        let ret = try_libc!({
            let mut retval: i32 = 0;
            let status = occlum_ocall_ioctl(
                &mut retval as *mut i32,
                host_fd,
                cmd_num,
                &termios as *const Termios as *mut c_void,
                len,
            );
            assert!(status == sgx_status_t::SGX_SUCCESS);
            retval
        });
        Ok(ret)
    }
}

impl Termios {
    pub fn to_kernel_termios(&self) -> KernelTermios {
        let mut c_cc = [0; KERNEL_NCCS];
        c_cc.copy_from_slice(&self.c_cc[..KERNEL_NCCS]);

        KernelTermios {
            c_iflag: self.c_iflag,
            c_oflag: self.c_oflag,
            c_cflag: self.c_cflag,
            c_lflag: self.c_lflag,
            c_line: self.c_line,
            c_cc: c_cc,
        }
    }
}
