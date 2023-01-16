use super::*;

pub async fn do_rmdir(fs_path: &FsPath) -> Result<()> {
    debug!("rmdir: fs_path: {:?}", fs_path);

    let (dir, file_name) = {
        let current = current!();
        let fs = current.fs();
        fs.lookup_dir_and_base_name(&fs_path.trim_end_matches('/'))
            .await?
    };
    let dentry = dir.find(&file_name).await?;
    if dentry.inode().metadata().await?.type_ != FileType::Dir {
        return_errno!(ENOTDIR, "rmdir on not directory");
    }
    dir.unlink(&file_name).await?;
    Ok(())
}
