use super::*;

pub fn do_mkdirat(fs_path: &FsPath, mode: FileMode) -> Result<()> {
    debug!("mkdirat: fs_path: {:?}, mode: {:#o}", fs_path, mode.bits());

    let path = fs_path.to_abs_path()?;
    let (dir_path, file_name) = split_path(&path);
    let current = current!();
    let inode = {
        let fs = current.fs().read().unwrap();
        fs.lookup_inode(dir_path)?
    };
    if inode.find(file_name).is_ok() {
        return_errno!(EEXIST, "");
    }
    if !inode.allow_write()? {
        return_errno!(EPERM, "dir cannot be written");
    }
    let masked_mode = mode & !current.process().umask();
    inode.create(file_name, FileType::Dir, masked_mode.bits())?;
    Ok(())
}
