use super::*;

pub async fn do_readlinkat(fs_path: &FsPath, buf: &mut [u8]) -> Result<usize> {
    debug!("readlinkat: fs_path: {:?}", fs_path);

    let file_path = {
        let inode = {
            let current = current!();
            let fs = current.fs();
            fs.lookup_inode_no_follow(fs_path).await?
        };
        if inode.metadata().await?.type_ != FileType::SymLink {
            return_errno!(EINVAL, "not a symbolic link");
        }
        inode.read_link().await?
    };
    let len = file_path.len().min(buf.len());
    buf[0..len].copy_from_slice(&file_path.as_bytes()[0..len]);
    Ok(len)
}

pub async fn do_symlinkat(target: &str, link_path: &FsPath) -> Result<usize> {
    debug!("symlinkat: target: {}, link_path: {:?}", target, link_path);

    if target.is_empty() {
        return_errno!(ENOENT, "target is an empty string");
    }
    if target.len() > PATH_MAX {
        return_errno!(ENAMETOOLONG, "target is too long");
    }

    let (dir_inode, link_name) = {
        let current = current!();
        let fs = current.fs();
        if link_path.ends_with("/") {
            return_errno!(EISDIR, "link path is dir");
        }
        fs.lookup_dirinode_and_basename(link_path).await?
    };
    if !dir_inode.allow_write() {
        return_errno!(EPERM, "symlink cannot be created");
    }
    let link_inode = dir_inode
        .create(&link_name, FileType::SymLink, 0o0777)
        .await?;
    link_inode.write_link(target).await?;
    Ok(0)
}
