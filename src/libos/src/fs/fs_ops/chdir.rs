use super::*;

pub fn do_chdir(path: &str) -> Result<()> {
    debug!("chdir: path: {:?}", path);

    let current = current!();
    let mut fs = current.fs().lock().unwrap();

    let inode = fs.lookup_inode(path)?;
    let info = inode.metadata()?;
    if info.type_ != FileType::Dir {
        return_errno!(ENOTDIR, "cwd must be directory");
    }

    fs.set_cwd(path)?;
    Ok(())
}
