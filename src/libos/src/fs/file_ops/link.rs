use super::*;

pub async fn do_linkat(old_fs_path: &FsPath, new_fs_path: &FsPath, flags: LinkFlags) -> Result<()> {
    debug!(
        "linkat: old_fs_path: {:?}, new_fs_path: {:?}, flags:{:?}",
        old_fs_path, new_fs_path, flags
    );

    let (dentry, new_dir, new_file_name) = {
        let current = current!();
        let fs = current.fs();
        let dentry = if flags.contains(LinkFlags::AT_SYMLINK_FOLLOW) {
            fs.lookup(old_fs_path).await?
        } else {
            fs.lookup_no_follow(old_fs_path).await?
        };
        if new_fs_path.ends_with("/") {
            return_errno!(EISDIR, "new path is dir");
        }
        let (new_dir, new_file_name) = fs.lookup_dir_and_base_name(new_fs_path).await?;
        (dentry, new_dir, new_file_name)
    };
    new_dir.link(&new_file_name, &dentry).await?;
    Ok(())
}

bitflags::bitflags! {
    pub struct LinkFlags: i32 {
        const AT_EMPTY_PATH = 0x1000;
        const AT_SYMLINK_FOLLOW = 0x400;
    }
}
