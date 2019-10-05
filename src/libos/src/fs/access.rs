use super::*;

//int faccessat(int dirfd, const char *pathname, int mode, int flags);
//int access(const char *pathname, int mode);

bitflags! {
    pub struct AccessModes : u32 {
        const X_OK = 1;
        const W_OK = 2;
        const R_OK = 4;
    }
}

impl AccessModes {
    pub fn from_u32(bits: u32) -> Result<AccessModes> {
        AccessModes::from_bits(bits).ok_or_else(|| errno!(EINVAL, "invalid mode"))
    }
}

bitflags! {
    pub struct AccessFlags : u32 {
        const AT_SYMLINK_NOFOLLOW = 0x100;
        const AT_EACCESS          = 0x200;
    }
}

impl AccessFlags {
    pub fn from_u32(bits: u32) -> Result<AccessFlags> {
        AccessFlags::from_bits(bits).ok_or_else(|| errno!(EINVAL, "invalid flags"))
    }
}

pub const AT_FDCWD: i32 = -100;

pub fn do_faccessat(
    dirfd: Option<FileDesc>,
    path: &str,
    mode: AccessModes,
    flags: AccessFlags,
) -> Result<()> {
    info!(
        "faccessat: dirfd: {:?}, path: {:?}, mode: {:?}, flags: {:?}",
        dirfd, path, mode, flags
    );
    match dirfd {
        // TODO: handle dirfd
        Some(dirfd) => return_errno!(ENOSYS, "cannot accept dirfd"),
        None => do_access(path, mode),
    }
}

pub fn do_access(path: &str, mode: AccessModes) -> Result<()> {
    info!("access: path: {:?}, mode: {:?}", path, mode);
    let current_ref = process::get_current();
    let mut current = current_ref.lock().unwrap();
    let inode = current.lookup_inode(path)?;
    //let metadata = inode.get_metadata();
    // TODO: check metadata.mode with mode
    Ok(())
}
