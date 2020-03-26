use super::*;

pub fn do_chdir(path: &str) -> Result<()> {
    debug!("chdir: path: {:?}", path);

    let current_ref = process::get_current();
    let mut current_process = current_ref.lock().unwrap();

    let inode = current_process.lookup_inode(path)?;
    let info = inode.metadata()?;
    if info.type_ != FileType::Dir {
        return_errno!(ENOTDIR, "");
    }

    current_process.change_cwd(path);
    Ok(())
}
