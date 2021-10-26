use super::*;

pub fn do_mkdirat(fs_path: &FsPath, mode: usize) -> Result<()> {
    debug!("mkdirat: fs_path: {:?}, mode: {:#o}", fs_path, mode);

    let (dir_inode, file_name) = {
        let current = current!();
        let fs = current.fs().lock().unwrap();
        fs.lookup_dirinode_and_basename(fs_path)?
    };
    if dir_inode.find(&file_name).is_ok() {
        return_errno!(EEXIST, "");
    }
    if !dir_inode.allow_write()? {
        return_errno!(EPERM, "dir cannot be written");
    }
    dir_inode.create(&file_name, FileType::Dir, mode as u32)?;
    Ok(())
}
