use super::*;

pub fn do_truncate(path: &str, len: usize) -> Result<()> {
    info!("truncate: path: {:?}, len: {}", path, len);
    let current_ref = process::get_current();
    let current_process = current_ref.lock().unwrap();
    current_process.lookup_inode(&path)?.resize(len)?;
    Ok(())
}

pub fn do_ftruncate(fd: FileDesc, len: usize) -> Result<()> {
    info!("ftruncate: fd: {}, len: {}", fd, len);
    let current_ref = process::get_current();
    let current_process = current_ref.lock().unwrap();
    let file_ref = current_process.get_files().lock().unwrap().get(fd)?;
    file_ref.set_len(len as u64)?;
    Ok(())
}
