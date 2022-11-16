use super::*;

pub async fn do_write(fd: FileDesc, buf: &[u8]) -> Result<usize> {
    debug!("write: fd: {}", fd);
    let file_ref = current!().file(fd)?;
    file_ref.write(buf).await
}

pub async fn do_writev(fd: FileDesc, bufs: &[&[u8]]) -> Result<usize> {
    debug!("writev: fd: {}", fd);
    let file_ref = current!().file(fd)?;
    file_ref.writev(bufs).await
}

pub async fn do_pwrite(fd: FileDesc, buf: &[u8], offset: off_t) -> Result<usize> {
    debug!("pwrite: fd: {}, offset: {}", fd, offset);
    if offset < 0 {
        return_errno!(EINVAL, "the offset is negative");
    }
    let file_ref = current!().file(fd)?;
    if let Some(async_file_handle) = file_ref.as_async_file_handle() {
        if !async_file_handle.access_mode().writable() {
            return_errno!(EBADF, "file is not writable");
        }
        async_file_handle
            .dentry()
            .inode()
            .write_at(offset as usize, buf)
            .await
    } else {
        // For non-inode files, we simply ignore the offset
        file_ref.write(buf).await
    }
}
