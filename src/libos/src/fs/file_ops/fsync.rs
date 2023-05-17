use super::*;

pub async fn do_fsync(fd: FileDesc) -> Result<()> {
    debug!("fsync: fd: {}", fd);
    let file_ref = current!().file(fd)?;
    if let Some(disk_file) = file_ref.as_disk_file() {
        disk_file.flush().await?;
    } else if let Some(async_file_handle) = file_ref.as_async_file_handle() {
        flush_vm_backed_by(&file_ref).await;
        async_file_handle.dentry().inode().sync_all().await?;
    } else {
        return_errno!(EBADF, "not supported");
    }
    Ok(())
}

pub async fn do_fdatasync(fd: FileDesc) -> Result<()> {
    debug!("fdatasync: fd: {}", fd);
    let file_ref = current!().file(fd)?;
    if let Some(disk_file) = file_ref.as_disk_file() {
        disk_file.flush().await?;
    } else if let Some(async_file_handle) = file_ref.as_async_file_handle() {
        flush_vm_backed_by(&file_ref).await;
        async_file_handle.dentry().inode().sync_data().await?;
    } else {
        return_errno!(EBADF, "not supported");
    }
    Ok(())
}

async fn flush_vm_backed_by(file: &FileRef) {
    current!().vm().msync_by_file(file).await;
}
