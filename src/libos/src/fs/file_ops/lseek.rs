use super::*;

pub fn do_lseek(fd: FileDesc, offset: SeekFrom) -> Result<usize> {
    let file_ref = current!().file(fd)?;
    file_ref.seek(offset)
}
