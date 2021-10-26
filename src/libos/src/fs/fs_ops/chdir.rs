use super::*;
use std::convert::TryFrom;

pub fn do_chdir(path: &str) -> Result<()> {
    debug!("chdir: path: {:?}", path);

    let current = current!();
    let mut fs = current.fs().lock().unwrap();

    let inode = fs.lookup_inode(&FsPath::try_from(path)?)?;
    let info = inode.metadata()?;
    if info.type_ != FileType::Dir {
        return_errno!(ENOTDIR, "cwd must be directory");
    }

    fs.set_cwd(path)?;
    Ok(())
}
