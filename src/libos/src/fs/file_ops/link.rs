use super::*;

pub fn do_link(oldpath: &str, newpath: &str) -> Result<()> {
    debug!("link: oldpath: {:?}, newpath: {:?}", oldpath, newpath);

    let (new_dir_path, new_file_name) = split_path(&newpath);
    let (inode, new_dir_inode) = {
        let current = current!();
        let fs = current.fs().lock().unwrap();
        let inode = fs.lookup_inode_no_follow(&oldpath)?;
        let new_dir_inode = fs.lookup_inode(new_dir_path)?;
        (inode, new_dir_inode)
    };
    new_dir_inode.link(new_file_name, &inode)?;
    Ok(())
}
