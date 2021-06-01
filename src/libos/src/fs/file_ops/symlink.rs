use super::*;

pub fn do_readlinkat(fs_path: &FsPath, buf: &mut [u8]) -> Result<usize> {
    debug!("readlinkat: fs_path: {:?}", fs_path);

    let path = fs_path.to_abs_path()?;
    let file_path = {
        let inode = {
            let current = current!();
            let fs = current.fs().read().unwrap();
            fs.lookup_inode_no_follow(&path)?
        };
        if inode.metadata()?.type_ != FileType::SymLink {
            return_errno!(EINVAL, "not a symbolic link");
        }
        let mut content = vec![0u8; PATH_MAX];
        let len = inode.read_at(0, &mut content)?;
        let path =
            std::str::from_utf8(&content[..len]).map_err(|_| errno!(EINVAL, "invalid symlink"))?;
        String::from(path)
    };
    let len = file_path.len().min(buf.len());
    buf[0..len].copy_from_slice(&file_path.as_bytes()[0..len]);
    Ok(len)
}

pub fn do_symlinkat(target: &str, link_path: &FsPath) -> Result<usize> {
    debug!("symlinkat: target: {}, link_path: {:?}", target, link_path);

    if target.is_empty() {
        return_errno!(ENOENT, "target is an empty string");
    }
    if target.len() > PATH_MAX {
        return_errno!(ENAMETOOLONG, "target is too long");
    }

    let link_path = link_path.to_abs_path()?;
    let (dir_path, link_name) = split_path(&link_path);
    let dir_inode = {
        let current = current!();
        let fs = current.fs().read().unwrap();
        fs.lookup_inode(dir_path)?
    };
    if !dir_inode.allow_write()? {
        return_errno!(EPERM, "symlink cannot be created");
    }
    let link_inode = dir_inode.create(link_name, FileType::SymLink, 0o0777)?;
    let data = target.as_bytes();
    link_inode.resize(data.len())?;
    link_inode.write_at(0, data)?;
    Ok(0)
}
