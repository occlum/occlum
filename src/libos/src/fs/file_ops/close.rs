use super::*;

pub fn do_close(fd: FileDesc) -> Result<()> {
    debug!("close: fd: {}", fd);
    let current = current!();
    let mut files = current.files().lock().unwrap();
    let file = files.del(fd)?;
    // Deadlock note: EpollFile's drop method needs to access file table. So
    // if the drop method is invoked inside the del method, then there will be
    // a deadlock.
    // TODO: make FileTable a struct of internal mutability to avoid deadlock.
    drop(files);
    drop(file);
    Ok(())
}
