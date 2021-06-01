use super::*;

bitflags! {
    pub struct AccessibilityCheckMode : u32 {
        /// F_OK = 0, test for the existence of the file
        /// X_OK, test for execute permission
        const X_OK = 1;
        /// W_OK, test for write permission
        const W_OK = 2;
        /// R_OK, test for read permission
        const R_OK = 4;
    }
}

impl AccessibilityCheckMode {
    pub fn from_u32(bits: u32) -> Result<Self> {
        AccessibilityCheckMode::from_bits(bits).ok_or_else(|| errno!(EINVAL, "invalid mode"))
    }

    pub fn test_for_exist(&self) -> bool {
        self.bits == 0
    }
}

bitflags! {
    pub struct AccessibilityCheckFlags : u32 {
        /// If path is a symbolic link, do not dereference it
        const AT_SYMLINK_NOFOLLOW = 0x100;
        /// Perform access checks using the effective user and group IDs
        const AT_EACCESS          = 0x200;
    }
}

impl AccessibilityCheckFlags {
    pub fn from_u32(bits: u32) -> Result<Self> {
        AccessibilityCheckFlags::from_bits(bits).ok_or_else(|| errno!(EINVAL, "invalid flags"))
    }
}

pub fn do_faccessat(
    fs_path: &FsPath,
    mode: AccessibilityCheckMode,
    flags: AccessibilityCheckFlags,
) -> Result<()> {
    debug!(
        "faccessat: fs_path: {:?}, mode: {:?}, flags: {:?}",
        fs_path, mode, flags
    );

    let inode = {
        let path = fs_path.to_abs_path()?;
        let current = current!();
        let fs = current.fs().read().unwrap();
        if flags.contains(AccessibilityCheckFlags::AT_SYMLINK_NOFOLLOW) {
            fs.lookup_inode_no_follow(&path)?
        } else {
            fs.lookup_inode(&path)?
        }
    };
    if mode.test_for_exist() {
        return Ok(());
    }
    // Check the permissions of file owner
    let owner_file_mode = {
        let metadata = inode.metadata()?;
        AccessibilityCheckMode::from_u32((metadata.mode >> 6) as u32 & 0b111)?
    };
    if !owner_file_mode.contains(mode) {
        return_errno!(EACCES, "the requested access is denied");
    }
    Ok(())
}
