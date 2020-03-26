use super::*;

pub fn do_lseek(fd: FileDesc, offset: SeekFrom) -> Result<off_t> {
    let file_ref = process::get_file(fd)?;
    file_ref.seek(offset)
}
