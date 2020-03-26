use super::*;

pub fn do_mkdir(path: &str, mode: usize) -> Result<()> {
    // TODO: check pathname
    debug!("mkdir: path: {:?}, mode: {:#o}", path, mode);

    let (dir_path, file_name) = split_path(&path);
    let inode = {
        let current_ref = process::get_current();
        let current_process = current_ref.lock().unwrap();
        current_process.lookup_inode(dir_path)?
    };
    if inode.find(file_name).is_ok() {
        return_errno!(EEXIST, "");
    }
    if !inode.allow_write()? {
        return_errno!(EPERM, "dir cannot be written");
    }
    inode.create(file_name, FileType::Dir, mode as u32)?;
    Ok(())
}
