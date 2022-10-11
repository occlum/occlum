use super::*;

pub async fn do_fallocate(
    fd: FileDesc,
    flags: FallocateFlags,
    offset: usize,
    len: usize,
) -> Result<()> {
    debug!(
        "fallocate: fd: {}, flags: {:?}, offset: {}, len: {}",
        fd, flags, offset, len
    );
    let file_ref = current!().file(fd)?;
    if let Some(device_file) = file_ref.as_disk_file() {
        // do nothing
        warn!("disk_file does not support fallocate");
        Ok(())
    } else if let Some(async_file_handle) = file_ref.as_async_file_handle() {
        if !async_file_handle.access_mode().writable() {
            return_errno!(EBADF, "File is not opened for writing");
        }
        let mode = FallocateMode::from(flags);
        async_file_handle
            .dentry()
            .inode()
            .fallocate(&mode, offset, len)
            .await?;
        Ok(())
    } else {
        return_errno!(EBADF, "not supported");
    }
}
