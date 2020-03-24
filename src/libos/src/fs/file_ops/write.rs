use super::*;

pub fn do_write(fd: FileDesc, buf: &[u8]) -> Result<usize> {
    debug!("write: fd: {}", fd);
    let current_ref = process::get_current();
    let current_process = current_ref.lock().unwrap();
    let file_ref = current_process.get_files().lock().unwrap().get(fd)?;
    file_ref.write(buf)
}

pub fn do_writev(fd: FileDesc, bufs: &[&[u8]]) -> Result<usize> {
    debug!("writev: fd: {}", fd);
    let current_ref = process::get_current();
    let current_process = current_ref.lock().unwrap();
    let file_ref = current_process.get_files().lock().unwrap().get(fd)?;
    file_ref.writev(bufs)
}

pub fn do_pwrite(fd: FileDesc, buf: &[u8], offset: usize) -> Result<usize> {
    debug!("pwrite: fd: {}, offset: {}", fd, offset);
    let current_ref = process::get_current();
    let current_process = current_ref.lock().unwrap();
    let file_ref = current_process.get_files().lock().unwrap().get(fd)?;
    file_ref.write_at(offset, buf)
}
