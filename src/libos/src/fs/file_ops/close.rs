use super::*;

pub fn do_close(fd: FileDesc) -> Result<()> {
    info!("close: fd: {}", fd);
    let current_ref = process::get_current();
    let current_process = current_ref.lock().unwrap();
    let file_table_ref = current_process.get_files();
    let mut file_table = file_table_ref.lock().unwrap();
    file_table.del(fd)?;
    Ok(())
}
