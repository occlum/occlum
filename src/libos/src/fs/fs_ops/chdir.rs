use super::*;
use std::convert::TryFrom;

pub async fn do_chdir(path: &str) -> Result<()> {
    debug!("chdir: path: {:?}", path);

    let current = current!();
    let inode = {
        let fs = current.fs();
        fs.lookup_inode(&FsPath::try_from(path)?).await?
    };
    if inode.metadata().await?.type_ != FileType::Dir {
        return_errno!(ENOTDIR, "cwd must be directory");
    }

    current.fs().set_cwd(path)?;
    Ok(())
}

pub async fn do_fchdir(fd: FileDesc) -> Result<()> {
    debug!("fchdir: fd: {}", fd);

    let current = current!();
    let file_ref = current.file(fd)?;
    let path = if let Some(inode_file) = file_ref.as_inode_file() {
        if inode_file.inode().metadata()?.type_ != FileType::Dir {
            return_errno!(ENOTDIR, "cwd must be directory");
        }
        inode_file.open_path()
    } else if let Some(async_file_handle) = file_ref.as_async_file_handle() {
        if async_file_handle.dentry().inode().metadata().await?.type_ != FileType::Dir {
            return_errno!(ENOTDIR, "cwd must be directory");
        }
        async_file_handle.dentry().abs_path()
    } else {
        return_errno!(EBADF, "not an inode");
    };
    current.fs().set_cwd(path)?;
    Ok(())
}
