use super::*;

pub async fn do_read(fd: FileDesc, buf: &mut [u8]) -> Result<usize> {
    debug!("read: fd: {}", fd);
    let file_ref = current!().file(fd)?;
    file_ref.read(buf).await
}

pub async fn do_readv(fd: FileDesc, bufs: &mut [&mut [u8]]) -> Result<usize> {
    debug!("readv: fd: {}", fd);
    let file_ref = current!().file(fd)?;
    file_ref.readv(bufs).await
}

pub async fn do_pread(fd: FileDesc, buf: &mut [u8], offset: off_t) -> Result<usize> {
    debug!("pread: fd: {}, offset: {}", fd, offset);
    if offset < 0 {
        return_errno!(EINVAL, "the offset is negative");
    }
    let file_ref = current!().file(fd)?;
    if let Some(async_file_handle) = file_ref.as_async_file_handle() {
        async_file_handle.pread(buf, offset as usize).await
    } else {
        // For non-inode files, we simply ignore the offset
        file_ref.read(buf).await
    }
}

pub async fn do_preadv(fd: FileDesc, bufs: &mut [&mut [u8]], offset: off_t) -> Result<usize> {
    debug!("preadv: fd: {}, offset {}", fd, offset);
    if offset < 0 {
        return_errno!(EINVAL, "the offset is negative");
    }
    let file_ref = current!().file(fd)?;
    if let Some(async_file_handle) = file_ref.as_async_file_handle() {
        async_file_handle.preadv(bufs, offset as usize).await
    } else {
        // For non-inode files, we simply return error
        return_errno!(EINVAL, "Do not support preadv");
    }
}
