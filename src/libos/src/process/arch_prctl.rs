use super::*;

#[allow(non_camel_case_types)]
#[derive(Debug)]
pub enum ArchPrctlCode {
    ARCH_SET_GS = 0x1001,
    ARCH_SET_FS = 0x1002,
    ARCH_GET_FS = 0x1003,
    ARCH_GET_GS = 0x1004,
}

impl ArchPrctlCode {
    pub fn from_u32(bits: u32) -> Result<ArchPrctlCode, Error> {
        match bits {
            0x1001 => Ok(ArchPrctlCode::ARCH_SET_GS),
            0x1002 => Ok(ArchPrctlCode::ARCH_SET_FS),
            0x1003 => Ok(ArchPrctlCode::ARCH_GET_FS),
            0x1004 => Ok(ArchPrctlCode::ARCH_GET_GS),
            _ => errno!(EINVAL, "Unknown code for arch_prctl"),
        }
    }
}

pub fn do_arch_prctl(code: ArchPrctlCode, addr: *mut usize) -> Result<(), Error> {
    info!("do_arch_prctl: code: {:?}, addr: {:#o}", code, addr as usize);
    match code {
       ArchPrctlCode::ARCH_SET_FS => {
            let current_ref = get_current();
            let mut current = current_ref.lock().unwrap();
            let task = &mut current.task;
            task.user_fsbase_addr = addr as usize;
       },
       ArchPrctlCode::ARCH_GET_FS => {
            let current_ref = get_current();
            let current = current_ref.lock().unwrap();
            let task = &current.task;
            unsafe { *addr = task.user_fsbase_addr; }
       },
       ArchPrctlCode::ARCH_SET_GS | ArchPrctlCode::ARCH_GET_GS
           => return errno!(EINVAL, "GS cannot be accessed from the user space"),
    }
    Ok(())
}
