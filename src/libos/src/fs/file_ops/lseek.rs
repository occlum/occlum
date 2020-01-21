use super::*;

pub fn do_lseek(fd: FileDesc, offset: SeekFrom) -> Result<off_t> {
    let current_ref = process::get_current();
    let current_process = current_ref.lock().unwrap();
    let file_ref = current_process.get_files().lock().unwrap().get(fd)?;
    file_ref.seek(offset)
}
