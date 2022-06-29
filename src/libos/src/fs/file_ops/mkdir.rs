use super::*;

pub async fn do_mkdirat(fs_path: &FsPath, mode: FileMode) -> Result<()> {
    debug!("mkdirat: fs_path: {:?}, mode: {:#o}", fs_path, mode);

    let (dir_inode, file_name) = {
        let current = current!();
        let fs = current.fs();
        fs.lookup_dirinode_and_basename(fs_path).await?
    };
    if dir_inode.find(&file_name).await.is_ok() {
        return_errno!(EEXIST, "");
    }
    if !dir_inode.allow_write() {
        return_errno!(EPERM, "dir cannot be written");
    }
    let masked_mode = mode & !current!().process().umask();
    dir_inode
        .create(&file_name, FileType::Dir, masked_mode.bits())
        .await?;
    Ok(())
}
