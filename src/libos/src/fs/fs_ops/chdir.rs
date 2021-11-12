use super::*;
use std::convert::TryFrom;

pub fn do_chdir(path: &str) -> Result<()> {
    debug!("chdir: path: {:?}", path);

    let current = current!();
    let inode = {
        let fs = current.fs().read().unwrap();
        fs.lookup_inode(&FsPath::try_from(path)?)?
    };
    if inode.metadata()?.type_ != FileType::Dir {
        return_errno!(ENOTDIR, "cwd must be directory");
    }

    current.fs().write().unwrap().set_cwd(path)?;
    Ok(())
}

pub fn do_fchdir(fd: FileDesc) -> Result<()> {
    debug!("fchdir: fd: {}", fd);

    let current = current!();
    let file_ref = current.file(fd)?;
    let inode_file = file_ref
        .as_inode_file()
        .ok_or_else(|| errno!(EBADF, "not an inode"))?;
    if inode_file.inode().metadata()?.type_ != FileType::Dir {
        return_errno!(ENOTDIR, "cwd must be directory");
    }
    let path = inode_file.open_path();
    current.fs().write().unwrap().set_cwd(path)?;
    Ok(())
}
