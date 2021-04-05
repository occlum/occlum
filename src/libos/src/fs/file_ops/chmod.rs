use crate::fs::FileMode;
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
    let mut info = file_ref.metadata()?;
    info.mode = mode.bits();
    file_ref.set_metadata(&info)?;
    Ok(())
}
