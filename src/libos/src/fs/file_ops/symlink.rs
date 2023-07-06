use super::*;

pub async fn do_readlinkat(fs_path: &FsPath, buf: &mut [u8]) -> Result<usize> {
    debug!("readlinkat: fs_path: {:?}", fs_path);

    let linkpath = {
        let dentry = {
            let current = current!();
            let fs = current.fs();
            fs.lookup_no_follow(fs_path).await?
        };
        dentry.inode().read_link().await?
    };
    let len = linkpath.len().min(buf.len());
    buf[..len].copy_from_slice(&linkpath.as_bytes()[..len]);
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

    let (dir, link_name) = {
        let current = current!();
        let fs = current.fs();
        if link_path.ends_with("/") {
            return_errno!(EISDIR, "link path is dir");
        }
        fs.lookup_dir_and_base_name(link_path).await?
    };
    if !dir.inode().allow_write().await {
        return_errno!(EPERM, "symlink cannot be created");
    }
    let link_dentry = dir
        .create(
            &link_name,
            FileType::SymLink,
            FileMode::from_bits(0o0777).unwrap(),
        )
        .await?;
    link_dentry.inode().write_link(target).await?;
    Ok(0)
}
