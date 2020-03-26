use super::*;

pub fn do_truncate(path: &str, len: usize) -> Result<()> {
    debug!("truncate: path: {:?}, len: {}", path, len);
    let inode = {
        let current_ref = process::get_current();
        let current_process = current_ref.lock().unwrap();
        current_process.lookup_inode(&path)?
    };
    inode.resize(len)?;
    Ok(())
}

pub fn do_ftruncate(fd: FileDesc, len: usize) -> Result<()> {
    debug!("ftruncate: fd: {}, len: {}", fd, len);
    let file_ref = process::get_file(fd)?;
    file_ref.set_len(len as u64)?;
    Ok(())
}
