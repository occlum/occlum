use super::*;

pub fn do_close(fd: FileDesc) -> Result<()> {
    debug!("close: fd: {}", fd);
    let current = current!();
    let mut files = current.files().lock().unwrap();
    files.del(fd)?;
    Ok(())
}
