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
        async_file_handle.pwrite(buf, offset as usize).await
    } else {
        // For non-inode files, we simply ignore the offset
        file_ref.write(buf).await
    }
}

pub async fn do_pwritev(fd: FileDesc, bufs: &[&[u8]], offset: off_t) -> Result<usize> {
    debug!("pwritev: fd: {}, offset: {}", fd, offset);
    if offset < 0 {
        return_errno!(EINVAL, "the offset is negative");
    }
    let file_ref = current!().file(fd)?;
    if let Some(async_file_handle) = file_ref.as_async_file_handle() {
        async_file_handle.pwritev(bufs, offset as usize).await
    } else {
        // For non-inode files, we simply return error
        return_errno!(EINVAL, "Do not support pwritev");
    }
}
