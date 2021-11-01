use super::*;

pub fn do_fallocate(fd: FileDesc, mode: u32, offset: u64, len: u64) -> Result<()> {
    debug!(
        "fallocate: fd: {}, mode: {}, offset: {}, len: {}",
        fd, mode, offset, len
    );
    let file_ref = current!().file(fd)?;
    let inode_file = file_ref
        .as_inode_file()
        .ok_or_else(|| errno!(EBADF, "not an inode"))?;
    if !inode_file.access_mode().writable() {
        return_errno!(EBADF, "File is not opened for writing");
    }
    inode_file.inode().fallocate(mode, offset, len)?;
    Ok(())
}
