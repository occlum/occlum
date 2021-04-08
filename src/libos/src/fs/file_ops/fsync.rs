use super::*;

pub async fn do_fsync(fd: FileDesc) -> Result<()> {
    debug!("fsync: fd: {}", fd);
    let file_ref = current!().file(fd)?;
    if let Some(inode) = file_ref.as_inode() {
        flush_vm_backed_by(&file_ref);
        inode.sync_all()?;
    } else {
        file_ref.flush().await;
    }
    Ok(())
}

pub async fn do_fdatasync(fd: FileDesc) -> Result<()> {
    debug!("fdatasync: fd: {}", fd);
    let file_ref = current!().file(fd)?;
    if let Some(inode) = file_ref.as_inode() {
        flush_vm_backed_by(&file_ref);
        inode.sync_data()?;
    } else {
        file_ref.flush().await;
    }
    Ok(())
}

fn flush_vm_backed_by(file: &FileRef) {
    current!().vm().msync_by_file(file);
}
