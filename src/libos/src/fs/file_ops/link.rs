use super::*;

pub async fn do_linkat(old_fs_path: &FsPath, new_fs_path: &FsPath, flags: LinkFlags) -> Result<()> {
    debug!(
        "linkat: old_fs_path: {:?}, new_fs_path: {:?}, flags:{:?}",
        old_fs_path, new_fs_path, flags
    );

    let (inode, new_dir_inode, new_file_name) = {
        let current = current!();
        let fs = current.fs();
        let inode = if flags.contains(LinkFlags::AT_SYMLINK_FOLLOW) {
            fs.lookup_inode(old_fs_path).await?
        } else {
            fs.lookup_inode_no_follow(old_fs_path).await?
        };
        let (new_dir_inode, new_file_name) = fs.lookup_dirinode_and_basename(new_fs_path).await?;
        (inode, new_dir_inode, new_file_name)
    };
    new_dir_inode.link(&new_file_name, &inode).await?;
    Ok(())
}

bitflags::bitflags! {
    pub struct LinkFlags: i32 {
        const AT_EMPTY_PATH = 0x1000;
        const AT_SYMLINK_FOLLOW = 0x400;
    }
}
