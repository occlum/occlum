use super::*;

bitflags! {
    pub struct ChownFlags: i32 {
        const AT_EMPTY_PATH = 0x1000;
        const AT_SYMLINK_NOFOLLOW = 0x100;
    }
}

pub fn do_fchownat(fs_path: &FsPath, uid: i32, gid: i32, flags: ChownFlags) -> Result<()> {
    debug!(
        "fchownat: fs_path: {:?}, uid: {}, gid: {}, flags: {:?}",
        fs_path, uid, gid, flags
    );

    let uid = to_opt(uid)?;
    let gid = to_opt(gid)?;
    // Return early if owner and group are -1
    if uid.is_none() && gid.is_none() {
        return Ok(());
    }

    let inode = {
        let path = fs_path.to_abs_path()?;
        let current = current!();
        let fs = current.fs().read().unwrap();
        if flags.contains(ChownFlags::AT_SYMLINK_NOFOLLOW) {
            fs.lookup_inode_no_follow(&path)?
        } else {
            fs.lookup_inode(&path)?
        }
    };
    let mut info = inode.metadata()?;
    if let Some(uid) = uid {
        info.uid = uid as usize;
    }
    if let Some(gid) = gid {
        info.gid = gid as usize;
    }
    inode.set_metadata(&info)?;
    Ok(())
}

pub fn do_fchown(fd: FileDesc, uid: i32, gid: i32) -> Result<()> {
    debug!("fchown: fd: {}, uid: {}, gid: {}", fd, uid, gid);

    let uid = to_opt(uid)?;
    let gid = to_opt(gid)?;
    // Return early if owner and group are -1
    if uid.is_none() && gid.is_none() {
        return Ok(());
    }

    let file_ref = current!().file(fd)?;
    let mut info = file_ref.metadata()?;
    if let Some(uid) = uid {
        info.uid = uid as usize;
    }
    if let Some(gid) = gid {
        info.gid = gid as usize;
    }
    file_ref.set_metadata(&info)?;
    Ok(())
}

fn to_opt(id: i32) -> Result<Option<u32>> {
    let id = if id >= 0 {
        Some(id as u32)
    } else if id == -1 {
        // If the ID is specified as -1, then that ID is not changed
        None
    } else {
        return_errno!(EINVAL, "invalid id");
    };

    Ok(id)
}
