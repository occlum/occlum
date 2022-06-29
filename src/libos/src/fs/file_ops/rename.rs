use super::*;

pub async fn do_renameat(old_fs_path: &FsPath, new_fs_path: &FsPath) -> Result<()> {
    debug!(
        "renameat: old_fs_path: {:?}, new_fs_path: {:?}",
        old_fs_path, new_fs_path
    );

    let current = current!();
    let fs = current.fs();

    let old_path = PathBuf::from(fs.convert_fspath_to_abs(old_fs_path)?);
    let new_path = PathBuf::from(fs.convert_fspath_to_abs(new_fs_path)?);
    // Limitation: only compare the whole path components, cannot handle symlink or ".."
    if new_path.starts_with(&old_path) && new_path != old_path {
        return_errno!(EINVAL, "newpath contains a path prefix of the oldpath");
    }

    let (old_dir_inode, old_file_name) = fs.lookup_dirinode_and_basename(old_fs_path).await?;
    let (new_dir_inode, new_file_name) = fs.lookup_dirinode_and_basename(new_fs_path).await?;
    let old_file_mode = {
        let old_file_inode = old_dir_inode.find(&old_file_name).await?;
        let metadata = old_file_inode.metadata().await?;
        FileMode::from_bits_truncate(metadata.mode)
    };
    if old_file_mode.has_sticky_bit() {
        warn!("ignoring the sticky bit");
    }
    // TODO: support to modify file's absolute path
    old_dir_inode
        .move_(&old_file_name, &new_dir_inode, &new_file_name)
        .await?;
    Ok(())
}
