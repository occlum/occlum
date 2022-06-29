use super::*;

pub async fn do_rmdir(fs_path: &FsPath) -> Result<()> {
    debug!("rmdir: fs_path: {:?}", fs_path);

    let (dir_inode, file_name) = {
        let current = current!();
        let fs = current.fs();
        fs.lookup_dirinode_and_basename(fs_path).await?
    };
    let file_inode = dir_inode.find(&file_name).await?;
    if file_inode.metadata().await?.type_ != FileType::Dir {
        return_errno!(ENOTDIR, "rmdir on not directory");
    }
    dir_inode.unlink(&file_name).await?;
    Ok(())
}
