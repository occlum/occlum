use super::*;

pub async fn do_truncate(fs_path: &FsPath, len: usize) -> Result<()> {
    debug!("truncate: path: {:?}, len: {}", fs_path, len);
    let inode = {
        let current = current!();
        let fs = current.fs();
        fs.lookup_inode(fs_path).await?
    };
    inode.resize(len).await?;
    Ok(())
}

pub async fn do_ftruncate(fd: FileDesc, len: usize) -> Result<()> {
    debug!("ftruncate: fd: {}, len: {}", fd, len);
    let file_ref = current!().file(fd)?;
    if let Some(inode_file) = file_ref.as_inode_file() {
        if !inode_file.access_mode().writable() {
            return_errno!(EBADF, "file is not opened for writing");
        }
        inode_file.inode().resize(len)?;
    } else if let Some(async_file_handle) = file_ref.as_async_file_handle() {
        if !async_file_handle.access_mode().writable() {
            return_errno!(EBADF, "file is not opened for writing");
        }
        async_file_handle.dentry().inode().resize(len).await?;
    } else if let Some(disk_file) = file_ref.as_disk_file() {
        warn!("disk_file does not support ftruncate");
    } else {
        return_errno!(EBADF, "not supported");
    }
    Ok(())
}
