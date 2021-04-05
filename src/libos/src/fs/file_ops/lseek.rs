use super::*;

pub fn do_lseek(fd: FileDesc, offset: SeekFrom) -> Result<usize> {
    debug!("lseek: fd: {:?}, offset: {:?}", fd, offset);
    let file_ref = current!().file(fd)?;
    file_ref.seek(offset)
}
