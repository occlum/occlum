use super::*;

pub fn do_fchmodat(fs_path: &FsPath, mode: FileMode) -> Result<()> {
    debug!("fchmodat: fs_path: {:?}, mode: {:#o}", fs_path, mode);

    let inode = {
        let path = fs_path.to_abs_path()?;
        let current = current!();
        let fs = current.fs().lock().unwrap();
        fs.lookup_inode(&path)?
    };
    let mut info = inode.metadata()?;
    info.mode = mode.bits();
    inode.set_metadata(&info)?;
    Ok(())
}

pub fn do_fchmod(fd: FileDesc, mode: FileMode) -> Result<()> {
    debug!("fchmod: fd: {}, mode: {:#o}", fd, mode);

    let file_ref = current!().file(fd)?;
    let inode_file = file_ref
        .as_inode_file()
        .ok_or_else(|| errno!(EINVAL, "not an inode"))?;
    let inode = inode_file.inode();
    let mut info = inode.metadata()?;
    info.mode = mode.bits();
    inode.set_metadata(&info)?;
    Ok(())
}
