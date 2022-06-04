use super::*;

pub async fn do_close(fd: FileDesc) -> Result<()> {
    debug!("close: fd: {}", fd);
    let current = current!();

    // Make sure the writes of disk files persist.
    //
    // Currently, disk files are the only types of files
    // that may have internal caches for updates and
    // requires explict flushes to ensure the persist of the
    // updates.
    //
    // TODO: add a general-purpose mechanism to do async drop.
    // If we can support async drop, then there is no need to
    // do explicit cleanup/shutdown/flush when closing fd.
    let file_ref = current!().file(fd)?;
    if let Some(disk_file) = file_ref.as_disk_file() {
        let _ = disk_file.flush().await;
    }

    current.close_file(fd)?;
    Ok(())
}
