use super::*;

pub async fn do_close(fd: FileDesc) -> Result<()> {
    debug!("close: fd: {}", fd);
    let current = current!();

    let file = current.file(fd)?;
    current.remove_file(fd)?;
    file.clean_for_close().await?;
    Ok(())
}
