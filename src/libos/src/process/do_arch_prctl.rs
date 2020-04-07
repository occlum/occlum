use crate::prelude::*;

pub fn do_arch_prctl(code: ArchPrctlCode, addr: *mut usize) -> Result<()> {
    debug!("do_arch_prctl: code: {:?}, addr: {:?}", code, addr);
    match code {
        ArchPrctlCode::ARCH_SET_FS => {
            current!().task().set_user_fs(addr as usize);
        }
        ArchPrctlCode::ARCH_GET_FS => unsafe {
            *addr = current!().task().user_fs();
        },
        ArchPrctlCode::ARCH_SET_GS | ArchPrctlCode::ARCH_GET_GS => {
            return_errno!(EINVAL, "GS cannot be accessed from the user space");
        }
    }
    Ok(())
}

#[allow(non_camel_case_types)]
#[derive(Debug)]
pub enum ArchPrctlCode {
    ARCH_SET_GS = 0x1001,
    ARCH_SET_FS = 0x1002,
    ARCH_GET_FS = 0x1003,
    ARCH_GET_GS = 0x1004,
}

impl ArchPrctlCode {
    pub fn from_u32(bits: u32) -> Result<ArchPrctlCode> {
        match bits {
            0x1001 => Ok(ArchPrctlCode::ARCH_SET_GS),
            0x1002 => Ok(ArchPrctlCode::ARCH_SET_FS),
            0x1003 => Ok(ArchPrctlCode::ARCH_GET_FS),
            0x1004 => Ok(ArchPrctlCode::ARCH_GET_GS),
            _ => return_errno!(EINVAL, "Unknown code for arch_prctl"),
        }
    }
}
