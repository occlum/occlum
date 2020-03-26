use super::*;

pub fn do_link(oldpath: &str, newpath: &str) -> Result<()> {
    debug!("link: oldpath: {:?}, newpath: {:?}", oldpath, newpath);

    let (new_dir_path, new_file_name) = split_path(&newpath);
    let (inode, new_dir_inode) = {
        let current_ref = process::get_current();
        let current_process = current_ref.lock().unwrap();
        let inode = current_process.lookup_inode(&oldpath)?;
        let new_dir_inode = current_process.lookup_inode(new_dir_path)?;
        (inode, new_dir_inode)
    };
    new_dir_inode.link(new_file_name, &inode)?;
    Ok(())
}
