use super::*;

pub async fn do_unlinkat(fs_path: &FsPath, flags: UnlinkFlags) -> Result<()> {
    debug!("unlinkat: fs_path: {:?}, flags: {:?}", fs_path, flags);

    if flags.contains(UnlinkFlags::AT_REMOVEDIR) {
        super::do_rmdir(fs_path).await?;
    } else {
        do_unlink(fs_path).await?;
    }
    Ok(())
}

bitflags::bitflags! {
    pub struct UnlinkFlags: i32 {
        const AT_REMOVEDIR = 0x200;
    }
}

async fn do_unlink(fs_path: &FsPath) -> Result<()> {
    let (dir_inode, file_name) = {
        let current = current!();
        let fs = current.fs();
        if fs_path.ends_with("/") {
            return_errno!(EISDIR, "unlink on directory");
        }
        fs.lookup_dirinode_and_basename(fs_path).await?
    };
    let file_inode = dir_inode.find(&file_name).await?;
    let metadata = file_inode.metadata().await?;
    if metadata.type_ == FileType::Dir {
        return_errno!(EISDIR, "unlink on directory");
    }
    if metadata.type_ == FileType::Socket {
        use host_socket::do_unlink as do_unlink_ocall;
        let mut path_buf = vec![0u8; PATH_MAX];
        let host_addr_len = file_inode.read_at(0, &mut path_buf).await?;
        let host_addr = String::from_utf8(path_buf[..host_addr_len].to_vec())
            .expect("The path string is invalid");
        do_unlink_ocall(&host_addr);
    }
    let file_mode = FileMode::from_bits_truncate(metadata.mode);
    if file_mode.has_sticky_bit() {
        warn!("ignoring the sticky bit");
    }
    dir_inode.unlink(&file_name).await?;
    Ok(())
}
