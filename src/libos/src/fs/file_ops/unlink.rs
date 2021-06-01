use super::*;

bitflags! {
    pub struct UnlinkFlags: i32 {
        const AT_REMOVEDIR = 0x200;
    }
}

fn do_unlink(path: &str) -> Result<()> {
    let (dir_path, file_name) = split_path(&path);
    let dir_inode = {
        let current = current!();
        let fs = current.fs().read().unwrap();
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

pub fn do_unlinkat(fs_path: &FsPath, flags: UnlinkFlags) -> Result<()> {
    debug!("unlinkat: fs_path: {:?}, flags: {:?}", fs_path, flags);

    let abs_path = fs_path.to_abs_path()?;
    if flags.contains(UnlinkFlags::AT_REMOVEDIR) {
        super::do_rmdir(&abs_path)
    } else {
        do_unlink(&abs_path)
    }
}
