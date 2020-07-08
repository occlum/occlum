use super::*;

pub fn do_fsync(fd: FileDesc) -> Result<()> {
    debug!("fsync: fd: {}", fd);
    let file_ref = current!().file(fd)?;
    flush_vm_backed_by(&file_ref);
    file_ref.sync_all()?;
    Ok(())
}

pub fn do_fdatasync(fd: FileDesc) -> Result<()> {
    debug!("fdatasync: fd: {}", fd);
    let file_ref = current!().file(fd)?;
    flush_vm_backed_by(&file_ref);
    file_ref.sync_data()?;
    Ok(())
}

fn flush_vm_backed_by(file: &FileRef) {
    current!().vm().msync_by_file(file);
}
