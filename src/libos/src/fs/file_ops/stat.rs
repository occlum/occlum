use super::builtin_disk::try_open_disk;
use super::*;

pub async fn do_fstat(fd: u32) -> Result<StatBuf> {
    debug!("fstat: fd: {}", fd);
    let file_ref = current!().file(fd as FileDesc)?;
    let stat = if let Some(async_file) = file_ref.as_async_file() {
        async_file.stat()
    } else if let Some(disk_file) = file_ref.as_disk_file() {
        StatBuf::from(disk_file.metadata())
    } else if let Some(async_file_handle) = file_ref.as_async_file_handle() {
        StatBuf::from(async_file_handle.dentry().inode().metadata().await?)
    } else {
        // TODO: support the stat operation on non-inode files
        return_errno!(ENODEV, "the file is not inode");
    };
    Ok(stat)
}

pub async fn do_fstatat(fs_path: &FsPath, flags: StatFlags) -> Result<StatBuf> {
    debug!("fstatat: fs_path: {:?}, flags: {:?}", fs_path, flags);

    let current = current!();
    let fs = current.fs();

    let stat = if let Some(disk_file) = try_open_disk(&fs, fs_path).await? {
        StatBuf::from(disk_file.metadata())
    } else {
        let inode = if flags.contains(StatFlags::AT_SYMLINK_NOFOLLOW) {
            fs.lookup_inode_no_follow(fs_path).await?
        } else {
            fs.lookup_inode(fs_path).await?
        };
        StatBuf::from(inode.metadata().await?)
    };

    Ok(stat)
}
