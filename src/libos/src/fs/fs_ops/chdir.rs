use super::*;
use std::convert::TryFrom;

pub async fn do_chdir(path: &str) -> Result<()> {
    debug!("chdir: path: {:?}", path);

    let current = current!();
    let dentry = {
        let fs = current.fs();
        fs.lookup(&FsPath::try_from(path)?).await?
    };
    if dentry.inode().metadata().await?.type_ != FileType::Dir {
        return_errno!(ENOTDIR, "cwd must be directory");
    }

    current.fs().set_cwd(dentry);
    Ok(())
}

pub async fn do_fchdir(fd: FileDesc) -> Result<()> {
    debug!("fchdir: fd: {}", fd);

    let current = current!();
    let file_ref = current.file(fd)?;
    let dentry = if let Some(async_file_handle) = file_ref.as_async_file_handle() {
        if async_file_handle.dentry().inode().metadata().await?.type_ != FileType::Dir {
            return_errno!(ENOTDIR, "cwd must be directory");
        }
        async_file_handle.dentry().clone()
    } else {
        return_errno!(EBADF, "not an inode");
    };
    current.fs().set_cwd(dentry);
    Ok(())
}
