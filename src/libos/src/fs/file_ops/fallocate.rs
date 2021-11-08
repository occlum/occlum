use super::*;

pub fn do_fallocate(fd: FileDesc, flags: FallocateFlags, offset: usize, len: usize) -> Result<()> {
    debug!(
        "fallocate: fd: {}, flags: {:?}, offset: {}, len: {}",
        fd, flags, offset, len
    );
    let file_ref = current!().file(fd)?;
    let inode_file = file_ref
        .as_inode_file()
        .ok_or_else(|| errno!(EBADF, "not an inode"))?;
    if !inode_file.access_mode().writable() {
        return_errno!(EBADF, "File is not opened for writing");
    }
    let mode = FallocateMode::from(flags);
    inode_file.inode().fallocate(&mode, offset, len)?;
    Ok(())
}
