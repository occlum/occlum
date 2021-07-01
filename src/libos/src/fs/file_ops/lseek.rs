use super::*;

pub fn do_lseek(fd: FileDesc, offset: SeekFrom) -> Result<usize> {
    debug!("lseek: fd: {:?}, offset: {:?}", fd, offset);
    let file_ref = current!().file(fd)?;
    let inode_file = file_ref
        .as_inode_file()
        .ok_or_else(|| errno!(EINVAL, "not an inode"))?;
    inode_file.seek(offset)
}
