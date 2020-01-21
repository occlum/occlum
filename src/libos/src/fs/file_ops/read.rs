use super::*;

pub fn do_read(fd: FileDesc, buf: &mut [u8]) -> Result<usize> {
    info!("read: fd: {}", fd);
    let current_ref = process::get_current();
    let current_process = current_ref.lock().unwrap();
    let file_ref = current_process.get_files().lock().unwrap().get(fd)?;
    file_ref.read(buf)
}

pub fn do_readv(fd: FileDesc, bufs: &mut [&mut [u8]]) -> Result<usize> {
    info!("readv: fd: {}", fd);
    let current_ref = process::get_current();
    let current_process = current_ref.lock().unwrap();
    let file_ref = current_process.get_files().lock().unwrap().get(fd)?;
    file_ref.readv(bufs)
}

pub fn do_pread(fd: FileDesc, buf: &mut [u8], offset: usize) -> Result<usize> {
    info!("pread: fd: {}, offset: {}", fd, offset);
    let current_ref = process::get_current();
    let current_process = current_ref.lock().unwrap();
    let file_ref = current_process.get_files().lock().unwrap().get(fd)?;
    file_ref.read_at(offset, buf)
}
