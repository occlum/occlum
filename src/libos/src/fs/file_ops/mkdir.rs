use super::*;

pub async fn do_mkdirat(fs_path: &FsPath, mode: FileMode) -> Result<()> {
    debug!("mkdirat: fs_path: {:?}, mode: {:#o}", fs_path, mode);

    let (dir, file_name) = {
        let current = current!();
        let fs = current.fs();
        fs.lookup_dir_and_base_name(&fs_path.trim_end_matches('/'))
            .await?
    };
    if dir.find(&file_name).await.is_ok() {
        return_errno!(EEXIST, "");
    }
    if !dir.inode().allow_write().await {
        return_errno!(EPERM, "dir cannot be written");
    }
    let masked_mode = mode & !current!().process().umask();
    dir.create(&file_name, FileType::Dir, masked_mode).await?;
    Ok(())
}
