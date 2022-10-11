use super::*;

bitflags! {
    pub struct ChownFlags: i32 {
        const AT_EMPTY_PATH = 0x1000;
        const AT_SYMLINK_NOFOLLOW = 0x100;
    }
}

pub async fn do_fchownat(fs_path: &FsPath, uid: u32, gid: u32, flags: ChownFlags) -> Result<()> {
    debug!(
        "fchownat: fs_path: {:?}, uid: {}, gid: {}, flags: {:?}",
        fs_path, uid, gid, flags
    );

    let inode = {
        let current = current!();
        let fs = current.fs();
        if flags.contains(ChownFlags::AT_SYMLINK_NOFOLLOW) {
            fs.lookup_inode_no_follow(fs_path).await?
        } else {
            fs.lookup_inode(fs_path).await?
        }
    };
    let mut info = inode.metadata().await?;
    info.uid = uid as usize;
    info.gid = gid as usize;
    inode.set_metadata(&info).await?;
    Ok(())
}

pub async fn do_fchown(fd: FileDesc, uid: u32, gid: u32) -> Result<()> {
    debug!("fchown: fd: {}, uid: {}, gid: {}", fd, uid, gid);

    let file_ref = current!().file(fd)?;
    if let Some(async_file_handle) = file_ref.as_async_file_handle() {
        let inode = async_file_handle.dentry().inode();
        let mut info = inode.metadata().await?;
        info.uid = uid as usize;
        info.gid = gid as usize;
        inode.set_metadata(&info).await?;
    } else {
        return_errno!(EBADF, "not an inode");
    }
    Ok(())
}
