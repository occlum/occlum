use super::*;
use crate::signal::{sigset_t, SigSet};
use crate::util::mem_util::from_user::check_ptr;

// Note: This is the Rust representation of `posix_spawnattr_t` defined in libc.
// The name of the elements follow the glibc style. The comments show the name in musl.
// Elements other than the listed ones are ignored because we don't care for now because
// Only POSIX_SPAWN_SETPGROUP, POSIX_SPAWN_SETSIGDEF and POSIX_SPAWN_SETSIGMASK are supported now.
#[repr(C)]
#[derive(Debug)]
pub struct posix_spawnattr_t {
    flags: SpawnAttributeFlags, // __flags
    pgrp: i32,                  // __pgrp
    sd: SpawnAttrSigSet,        // __def
    ss: SpawnAttrSigSet,        // __mask
}

// Glibc and musl use 128 bytes to represent sig_set_t
type SpawnAttrSigSet = [sigset_t; 16];

bitflags! {
    pub struct SpawnAttributeFlags: u16 {
        const POSIX_SPAWN_RESETIDS = 1; // 0x1
        const POSIX_SPAWN_SETPGROUP = 1 << 1; // 0x2
        const POSIX_SPAWN_SETSIGDEF = 1 << 2; // 0x4
        const POSIX_SPAWN_SETSIGMASK = 1 << 3; // 0x8
        const POSIX_SPAWN_SETSCHEDPARAM = 1 << 4; // 0x10
        const POSIX_SPAWN_SETSCHEDULER = 1 << 5; // 0x20
    }
}

impl SpawnAttributeFlags {
    fn supported(&self) -> bool {
        let unsupported_flags = SpawnAttributeFlags::all()
            - SpawnAttributeFlags::POSIX_SPAWN_SETPGROUP
            - SpawnAttributeFlags::POSIX_SPAWN_SETSIGDEF
            - SpawnAttributeFlags::POSIX_SPAWN_SETSIGMASK;
        if self.intersects(unsupported_flags) {
            false
        } else {
            true
        }
    }
}

#[derive(Default, Debug, Copy, Clone)]
pub struct SpawnAttr {
    pub process_group: Option<pid_t>,
    pub sig_mask: Option<SigSet>,
    pub sig_default: Option<SigSet>,
}

pub fn clone_spawn_attributes_safely(
    attr_ptr: *const posix_spawnattr_t,
) -> Result<Option<SpawnAttr>> {
    if attr_ptr != std::ptr::null() {
        check_ptr(attr_ptr)?;
    } else {
        return Ok(None);
    }

    let spawn_attr = unsafe { &*attr_ptr };
    let mut safe_attr = SpawnAttr::default();
    if spawn_attr.flags.is_empty() {
        return Ok(None);
    }

    if !spawn_attr.flags.supported() {
        warn!(
            "Unsupported flags contained. Attribute flags: {:?}",
            spawn_attr.flags
        );
    }

    if spawn_attr
        .flags
        .contains(SpawnAttributeFlags::POSIX_SPAWN_SETPGROUP)
    {
        safe_attr.process_group = Some(spawn_attr.pgrp as pid_t);
    }
    if spawn_attr
        .flags
        .contains(SpawnAttributeFlags::POSIX_SPAWN_SETSIGDEF)
    {
        safe_attr.sig_default = Some(SigSet::from_c(spawn_attr.sd[0]));
    }
    if spawn_attr
        .flags
        .contains(SpawnAttributeFlags::POSIX_SPAWN_SETSIGMASK)
    {
        safe_attr.sig_mask = Some(SigSet::from_c(spawn_attr.ss[0]));
    }

    Ok(Some(safe_attr))
}
