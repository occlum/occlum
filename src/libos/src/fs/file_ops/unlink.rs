use super::*;

pub fn do_unlink(path: &str) -> Result<()> {
    debug!("unlink: path: {:?}", path);

    let (dir_path, file_name) = split_path(&path);
    let dir_inode = {
        let current = current!();
        let fs = current.fs().lock().unwrap();
        fs.lookup_inode(dir_path)?
    };
    let file_inode = dir_inode.find(file_name)?;
    let metadata = file_inode.metadata()?;
    if metadata.type_ == FileType::Dir {
        return_errno!(EISDIR, "unlink on directory");
    }
    let file_mode = FileMode::from_bits_truncate(metadata.mode);
    if file_mode.has_sticky_bit() {
        warn!("ignoring the sticky bit");
    }
    dir_inode.unlink(file_name)?;
    Ok(())
}
