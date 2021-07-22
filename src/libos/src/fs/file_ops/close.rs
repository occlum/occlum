use super::*;

pub fn do_close(fd: FileDesc) -> Result<()> {
    debug!("close: fd: {}", fd);
    let current = current!();
    let file = current.del_file(fd)?;
    // Deadlock note: EpollFile's drop method needs to access file table. So
    // if the drop method is invoked inside the del method, then there will be
    // a deadlock.
    // TODO: make FileTable a struct of internal mutability to avoid deadlock.
    drop(file);
    Ok(())
}
