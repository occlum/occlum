use super::*;

pub fn do_renameat(old_fs_path: &FsPath, new_fs_path: &FsPath) -> Result<()> {
    debug!(
        "renameat: old_fs_path: {:?}, new_fs_path: {:?}",
        old_fs_path, new_fs_path
    );

    let oldpath = old_fs_path.to_abs_path()?;
    let newpath = new_fs_path.to_abs_path()?;

    let old_path = Path::new(&oldpath);
    let new_path = Path::new(&newpath);
    // Limitation: only compare the whole path components, cannot handle symlink or ".."
    if new_path.starts_with(old_path) && new_path != old_path {
        return_errno!(EINVAL, "newpath contains a path prefix of the oldpath");
    }

    let current = current!();
    let fs = current.fs().read().unwrap();

    let (old_dir_path, old_file_name) = split_path(&oldpath);
    let (new_dir_path, new_file_name) = split_path(&newpath);
    let old_dir_inode = fs.lookup_inode(old_dir_path)?;
    let new_dir_inode = fs.lookup_inode(new_dir_path)?;
    let old_file_mode = {
        let old_file_inode = old_dir_inode.find(old_file_name)?;
        let metadata = old_file_inode.metadata()?;
        FileMode::from_bits_truncate(metadata.mode)
    };
    if old_file_mode.has_sticky_bit() {
        warn!("ignoring the sticky bit");
    }
    // TODO: support to modify file's absolute path
    old_dir_inode.move_(old_file_name, &new_dir_inode, new_file_name)?;
    Ok(())
}
