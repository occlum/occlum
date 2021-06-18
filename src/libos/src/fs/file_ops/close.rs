use super::*;

pub fn do_close(fd: FileDesc) -> Result<()> {
    debug!("close: fd: {}", fd);
    let current = current!();
    current.close_file(fd)?;
    Ok(())
}
