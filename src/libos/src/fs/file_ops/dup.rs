use super::*;

pub fn do_dup(old_fd: FileDesc) -> Result<FileDesc> {
    let current_ref = process::get_current();
    let current = current_ref.lock().unwrap();
    let file_table_ref = current.get_files();
    let mut file_table = file_table_ref.lock().unwrap();
    let file = file_table.get(old_fd)?;
    let new_fd = file_table.put(file, false);
    Ok(new_fd)
}

pub fn do_dup2(old_fd: FileDesc, new_fd: FileDesc) -> Result<FileDesc> {
    let current_ref = process::get_current();
    let current = current_ref.lock().unwrap();
    let file_table_ref = current.get_files();
    let mut file_table = file_table_ref.lock().unwrap();
    let file = file_table.get(old_fd)?;
    if old_fd != new_fd {
        file_table.put_at(new_fd, file, false);
    }
    Ok(new_fd)
}

pub fn do_dup3(old_fd: FileDesc, new_fd: FileDesc, flags: u32) -> Result<FileDesc> {
    let creation_flags = CreationFlags::from_bits_truncate(flags);
    let current_ref = process::get_current();
    let current = current_ref.lock().unwrap();
    let file_table_ref = current.get_files();
    let mut file_table = file_table_ref.lock().unwrap();
    let file = file_table.get(old_fd)?;
    if old_fd == new_fd {
        return_errno!(EINVAL, "old_fd must not be equal to new_fd");
    }
    file_table.put_at(new_fd, file, creation_flags.must_close_on_spawn());
    Ok(new_fd)
}
