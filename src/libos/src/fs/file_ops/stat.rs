use super::builtin_disk::try_open_disk;
use super::*;

pub fn do_fstat(fd: u32) -> Result<StatBuf> {
    debug!("fstat: fd: {}", fd);
    let file_ref = current!().file(fd as FileDesc)?;
    if let Some(inode_file) = file_ref.as_inode_file() {
        let stat = StatBuf::from(inode_file.inode().metadata()?);
        Ok(stat)
    } else if let Some(async_file) = file_ref.as_async_file() {
        Ok(async_file.stat())
    } else if let Some(disk_file) = file_ref.as_disk_file() {
        let stat = StatBuf::from(disk_file.metadata());
        Ok(stat)
    } else {
        // TODO: support the stat operation on non-inode files
        return_errno!(ENODEV, "the file is not inode");
    }
}

pub fn do_fstatat(fs_path: &FsPath, flags: StatFlags) -> Result<StatBuf> {
    debug!("fstatat: fs_path: {:?}, flags: {:?}", fs_path, flags);

    let current = current!();
    let fs = current.fs().read().unwrap();

    let stat = if let Some(disk_file) = try_open_disk(&fs, fs_path)? {
        StatBuf::from(disk_file.metadata())
    } else {
        let inode = if flags.contains(StatFlags::AT_SYMLINK_NOFOLLOW) {
            fs.lookup_inode_no_follow(fs_path)?
        } else {
            fs.lookup_inode(fs_path)?
        };
        StatBuf::from(inode.metadata()?)
    };

    Ok(stat)
}
