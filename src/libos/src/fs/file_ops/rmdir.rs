use super::*;

pub fn do_rmdir(path: &str) -> Result<()> {
    debug!("rmdir: path: {:?}", path);

    let (dir_path, file_name) = split_path(&path);
    let dir_inode = {
        let current = current!();
        let fs = current.fs().read().unwrap();
        fs.lookup_inode(dir_path)?
    };
    let file_inode = dir_inode.find(file_name)?;
    if file_inode.metadata()?.type_ != FileType::Dir {
        return_errno!(ENOTDIR, "rmdir on not directory");
    }
    dir_inode.unlink(file_name)?;
    Ok(())
}
