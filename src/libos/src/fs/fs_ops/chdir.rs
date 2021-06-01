use super::*;

pub fn do_chdir(path: &str) -> Result<()> {
    debug!("chdir: path: {:?}", path);

    let current = current!();
    let inode = {
        let fs = current.fs().read().unwrap();
        fs.lookup_inode(path)?
    };
    let info = inode.metadata()?;
    if info.type_ != FileType::Dir {
        return_errno!(ENOTDIR, "cwd must be directory");
    }

    current.fs().write().unwrap().set_cwd(path)?;
    Ok(())
}
