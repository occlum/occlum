use super::*;

pub fn do_readlink(path: &str, buf: &mut [u8]) -> Result<usize> {
    debug!("readlink: path: {:?}", path);
    let file_path = {
        if path == "/proc/self/exe" {
            current!().process().exec_path().to_owned()
        } else if path.starts_with("/proc/self/fd") {
            let fd = path
                .trim_start_matches("/proc/self/fd/")
                .parse::<FileDesc>()
                .map_err(|e| errno!(EBADF, "Invalid file descriptor"))?;
            let file_ref = current!().file(fd)?;
            if let Ok(inode_file) = file_ref.as_inode_file() {
                inode_file.get_abs_path().to_owned()
            } else {
                // TODO: support special device files
                return_errno!(EINVAL, "not a normal file link")
            }
        } else {
            let inode = {
                let current = current!();
                let fs = current.fs().lock().unwrap();
                fs.lookup_inode_no_follow(path)?
            };
            if inode.metadata()?.type_ != FileType::SymLink {
                return_errno!(EINVAL, "not a symbolic link");
            }
            let mut content = vec![0u8; PATH_MAX];
            let len = inode.read_at(0, &mut content)?;
            let path = std::str::from_utf8(&content[..len])
                .map_err(|_| errno!(EINVAL, "invalid symlink"))?;
            String::from(path)
        }
    };
    let len = file_path.len().min(buf.len());
    buf[0..len].copy_from_slice(&file_path.as_bytes()[0..len]);
    Ok(len)
}

fn do_symlink(target: &str, link_path: &str) -> Result<usize> {
    let (dir_path, link_name) = split_path(&link_path);
    let dir_inode = {
        let current = current!();
        let fs = current.fs().lock().unwrap();
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

pub fn do_symlinkat(target: &str, new_dirfd: DirFd, link_path: &str) -> Result<usize> {
    debug!(
        "symlinkat: target: {:?}, new_dirfd: {:?}, link_path: {:?}",
        target, new_dirfd, link_path
    );
    if target.is_empty() || link_path.is_empty() {
        return_errno!(ENOENT, "target or linkpath is an empty string");
    }
    if target.len() > PATH_MAX || link_path.len() > PATH_MAX {
        return_errno!(ENAMETOOLONG, "target or linkpath is too long");
    }
    let link_path = match new_dirfd {
        DirFd::Fd(dirfd) => {
            let dir_path = get_dir_path(dirfd)?;
            dir_path + "/" + link_path
        }
        DirFd::Cwd => link_path.to_owned(),
    };
    do_symlink(target, &link_path)
}
