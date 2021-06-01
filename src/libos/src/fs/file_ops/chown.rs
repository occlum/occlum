use super::*;

bitflags! {
    pub struct ChownFlags: i32 {
        const AT_EMPTY_PATH = 0x1000;
        const AT_SYMLINK_NOFOLLOW = 0x100;
    }
}

pub fn do_fchownat(fs_path: &FsPath, uid: u32, gid: u32, flags: ChownFlags) -> Result<()> {
    debug!(
        "fchownat: fs_path: {:?}, uid: {}, gid: {}, flags: {:?}",
        fs_path, uid, gid, flags
    );

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
    info.uid = uid as usize;
    info.gid = gid as usize;
    inode.set_metadata(&info)?;
    Ok(())
}

pub fn do_fchown(fd: FileDesc, uid: u32, gid: u32) -> Result<()> {
    debug!("fchown: fd: {}, uid: {}, gid: {}", fd, uid, gid);

    let file_ref = current!().file(fd)?;
    let mut info = file_ref.metadata()?;
    info.uid = uid as usize;
    info.gid = gid as usize;
    file_ref.set_metadata(&info)?;
    Ok(())
}
