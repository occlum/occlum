use super::*;

pub fn do_truncate(fs_path: &FsPath, len: usize) -> Result<()> {
    debug!("truncate: path: {:?}, len: {}", fs_path, len);
    let inode = {
        let current = current!();
        let fs = current.fs().read().unwrap();
        fs.lookup_inode(fs_path)?
    };
    inode.resize(len)?;
    Ok(())
}

pub fn do_ftruncate(fd: FileDesc, len: usize) -> Result<()> {
    debug!("ftruncate: fd: {}, len: {}", fd, len);
    let file_ref = current!().file(fd)?;
    if let Some(inode_file) = file_ref.as_inode_file() {
        if !inode_file.access_mode().writable() {
            return_errno!(EBADF, "File is not opened for writing");
        }
        inode_file.inode().resize(len)?;
        Ok(())
    } else if let Some(disk_file) = file_ref.as_disk_file() {
        warn!("disk_file does not support ftruncate");
        Ok(())
    } else {
        return_errno!(EBADF, "not supported");
    }
}
