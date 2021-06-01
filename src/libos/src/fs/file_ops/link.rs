use super::*;

bitflags! {
    pub struct LinkFlags: i32 {
        const AT_EMPTY_PATH = 0x1000;
        const AT_SYMLINK_FOLLOW = 0x400;
    }
}

pub fn do_linkat(old_fs_path: &FsPath, new_fs_path: &FsPath, flags: LinkFlags) -> Result<()> {
    debug!(
        "linkat: old_fs_path: {:?}, new_fs_path: {:?}, flags:{:?}",
        old_fs_path, new_fs_path, flags
    );

    let newpath = new_fs_path.to_abs_path()?;
    let (new_dir_path, new_file_name) = split_path(&newpath);
    let (inode, new_dir_inode) = {
        let oldpath = old_fs_path.to_abs_path()?;
        let current = current!();
        let fs = current.fs().read().unwrap();
        let inode = if flags.contains(LinkFlags::AT_SYMLINK_FOLLOW) {
            fs.lookup_inode(&oldpath)?
        } else {
            fs.lookup_inode_no_follow(&oldpath)?
        };
        let new_dir_inode = fs.lookup_inode(new_dir_path)?;
        (inode, new_dir_inode)
    };
    new_dir_inode.link(new_file_name, &inode)?;
    Ok(())
}
