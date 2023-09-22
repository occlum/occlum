use crate::prelude::*;
use crate::util::mem_util::from_user::check_mut_ptr;

pub fn do_arch_prctl(code: ArchPrctlCode, addr: *mut usize) -> Result<()> {
    debug!("do_arch_prctl: code: {:?}, addr: {:?}", code, addr);
    match code {
        ArchPrctlCode::ARCH_SET_FS => {
            check_mut_ptr(addr)?;
            current!().task().set_user_fs(addr as usize);
        }
        ArchPrctlCode::ARCH_GET_FS => unsafe {
            check_mut_ptr(addr)?;
            *addr = current!().task().user_fs();
        },
        ArchPrctlCode::ARCH_SET_GS | ArchPrctlCode::ARCH_GET_GS => {
            check_mut_ptr(addr)?;
            return_errno!(EINVAL, "GS cannot be accessed from the user space");
        }
        ArchPrctlCode::ARCH_REQ_XCOMP_PERM => {
            // Allows to request permission for a dynamically enabled feature or a feature set
            // Currently only used to enable AMX
            use crate::util::sgx::get_self_target;
            const XFEATURE_XTILEDATA: u64 = 18;

            let features = addr as u64;
            if features == XFEATURE_XTILEDATA {
                // Check if AMX is enabled for current Enclave
                let target_info = get_self_target()?;
                if target_info.attributes.xfrm & SGX_XFRM_AMX != SGX_XFRM_AMX {
                    return_errno!(EINVAL, "AMX is not enabled for this enclave");
                } else {
                    info!("AMX is enabled for this enclave");
                }
            } else {
                return_errno!(ENOSYS, "feature not supported");
            }
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
    ARCH_REQ_XCOMP_PERM = 0x1023,
}

impl ArchPrctlCode {
    pub fn from_u32(bits: u32) -> Result<ArchPrctlCode> {
        match bits {
            0x1001 => Ok(ArchPrctlCode::ARCH_SET_GS),
            0x1002 => Ok(ArchPrctlCode::ARCH_SET_FS),
            0x1003 => Ok(ArchPrctlCode::ARCH_GET_FS),
            0x1004 => Ok(ArchPrctlCode::ARCH_GET_GS),
            0x1023 => Ok(ArchPrctlCode::ARCH_REQ_XCOMP_PERM),
            _ => return_errno!(EINVAL, "Unknown code for arch_prctl"),
        }
    }
}
