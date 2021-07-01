use super::*;

pub async fn do_fsync(fd: FileDesc) -> Result<()> {
    debug!("fsync: fd: {}", fd);
    let file_ref = current!().file(fd)?;
    let inode_file = file_ref
        .as_inode_file()
        .ok_or_else(|| errno!(EINVAL, "not an inode"))?;
    flush_vm_backed_by(&file_ref);
    inode_file.inode().sync_all()?;
    Ok(())
}

pub async fn do_fdatasync(fd: FileDesc) -> Result<()> {
    debug!("fdatasync: fd: {}", fd);
    let file_ref = current!().file(fd)?;
    let inode_file = file_ref
        .as_inode_file()
        .ok_or_else(|| errno!(EINVAL, "not an inode"))?;
    flush_vm_backed_by(&file_ref);
    inode_file.inode().sync_data()?;
    Ok(())
}

fn flush_vm_backed_by(file: &FileRef) {
    current!().vm().msync_by_file(file);
}
