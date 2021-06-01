use super::*;

pub fn do_renameat(old_fs_path: &FsPath, new_fs_path: &FsPath) -> Result<()> {
    debug!(
        "renameat: old_fs_path: {:?}, new_fs_path: {:?}",
        old_fs_path, new_fs_path
    );

    let current = current!();
    let fs = current.fs().read().unwrap();

    let (old_dir_inode, old_file_name) = fs.lookup_dirinode_and_basename(old_fs_path)?;
    let (new_dir_inode, new_file_name) = fs.lookup_dirinode_and_basename(new_fs_path)?;
    let old_file_mode = {
        let old_file_inode = old_dir_inode.find(&old_file_name)?;
        let metadata = old_file_inode.metadata()?;
        FileMode::from_bits_truncate(metadata.mode)
    };
    if old_file_mode.has_sticky_bit() {
        warn!("ignoring the sticky bit");
    }
    // TODO: support to modify file's absolute path
    old_dir_inode.move_(&old_file_name, &new_dir_inode, &new_file_name)?;
    Ok(())
}
