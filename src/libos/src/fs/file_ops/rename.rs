use super::*;

pub async fn do_renameat(old_fs_path: &FsPath, new_fs_path: &FsPath) -> Result<()> {
    debug!(
        "renameat: old_fs_path: {:?}, new_fs_path: {:?}",
        old_fs_path, new_fs_path
    );

    let current = current!();
    let fs = current.fs();

    let old_dentry = fs.lookup_no_follow(&old_fs_path).await?;
    let old_metadata = old_dentry.inode().metadata().await?;
    let old_file_mode = FileMode::from_bits_truncate(old_metadata.mode);
    if old_file_mode.has_sticky_bit() {
        warn!("ignoring the sticky bit");
    }

    let (new_dir, new_file_name) = {
        if new_fs_path.ends_with("/") && old_metadata.type_ != FileType::Dir {
            return_errno!(ENOTDIR, "oldpath is not dir");
        }
        fs.lookup_dir_and_base_name(&new_fs_path.trim_end_matches('/'))
            .await?
    };

    // Check the path before rename
    let old_abs_path = old_dentry.abs_path();
    let new_abs_path = new_dir.abs_path() + "/" + &new_file_name;
    if new_abs_path.starts_with(&old_abs_path) {
        if new_abs_path.len() == old_abs_path.len() {
            return Ok(());
        } else {
            return_errno!(EINVAL, "newpath contains a path prefix of the oldpath");
        }
    }

    let old_dir = old_dentry.parent().unwrap();
    old_dir
        .move_(&old_dentry.name(), &new_dir, &new_file_name)
        .await?;
    Ok(())
}
