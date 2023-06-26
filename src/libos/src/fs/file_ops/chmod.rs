use super::*;

pub async fn do_fchmodat(fs_path: &FsPath, mode: FileMode) -> Result<()> {
    debug!("fchmodat: fs_path: {:?}, mode: {:#o}", fs_path, mode);

    let inode = {
        let current = current!();
        let fs = current.fs();
        let dentry = fs.lookup(fs_path).await?;
        dentry.inode()
    };
    let mut info = inode.metadata().await?;
    info.mode = mode.bits();
    inode.set_metadata(&info).await?;
    Ok(())
}

pub async fn do_fchmod(fd: FileDesc, mode: FileMode) -> Result<()> {
    debug!("fchmod: fd: {}, mode: {:#o}", fd, mode);

    let file_ref = current!().file(fd)?;
    if let Some(async_file_handle) = file_ref.as_async_file_handle() {
        let inode = async_file_handle.dentry().inode();
        let mut info = inode.metadata().await?;
        info.mode = mode.bits();
        inode.set_metadata(&info).await?;
    } else {
        return_errno!(EBADF, "not an inode");
    }
    Ok(())
}
