use super::*;

pub fn do_chown(path: &str, uid: u32, gid: u32) -> Result<()> {
    warn!("chown is partial implemented as lchown");
    do_lchown(path, uid, gid)
}

pub fn do_fchown(fd: FileDesc, uid: u32, gid: u32) -> Result<()> {
    debug!("fchown: fd: {}, uid: {}, gid: {}", fd, uid, gid);
    let file_ref = process::get_file(fd)?;
    let mut info = file_ref.metadata()?;
    info.uid = uid as usize;
    info.gid = gid as usize;
    file_ref.set_metadata(&info)?;
    Ok(())
}

pub fn do_lchown(path: &str, uid: u32, gid: u32) -> Result<()> {
    debug!("lchown: path: {:?}, uid: {}, gid: {}", path, uid, gid);
    let inode = {
        let current_ref = process::get_current();
        let mut current = current_ref.lock().unwrap();
        current.lookup_inode(path)?
    };
    let mut info = inode.metadata()?;
    info.uid = uid as usize;
    info.gid = gid as usize;
    inode.set_metadata(&info)?;
    Ok(())
}
