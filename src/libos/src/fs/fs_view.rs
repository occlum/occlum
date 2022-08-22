/// Present a per-process view of FS.
use super::*;

use super::async_fs::ASYNC_SFS_NAME;
use super::fspath::FsPathInner;

#[derive(Debug)]
pub struct FsView {
    root: String,
    cwd: RwLock<String>,
}

impl Clone for FsView {
    fn clone(&self) -> Self {
        Self {
            root: self.root.clone(),
            cwd: RwLock::new(self.cwd()),
        }
    }
}

impl FsView {
    pub fn new() -> FsView {
        let root = String::from("/");
        let cwd = root.clone();
        Self {
            root,
            cwd: RwLock::new(cwd),
        }
    }

    /// Get the root directory
    pub fn root(&self) -> &str {
        &self.root
    }

    /// Get the current working directory.
    pub fn cwd(&self) -> String {
        self.cwd.read().unwrap().clone()
    }

    /// Set the current working directory.
    pub fn set_cwd(&self, path: &str) -> Result<()> {
        if path.len() == 0 {
            return_errno!(EINVAL, "empty path");
        }

        let mut cwd = self.cwd.write().unwrap();
        if path.starts_with("/") {
            // absolute
            *cwd = path.to_owned();
        } else {
            // relative
            if !cwd.ends_with("/") {
                *cwd += "/";
            }
            *cwd += path;
        }
        Ok(())
    }

    /// Open a file on the process. But DO NOT add it to file table.
    pub async fn open_file(&self, fs_path: &FsPath, flags: u32, mode: FileMode) -> Result<FileRef> {
        let creation_flags = CreationFlags::from_bits_truncate(flags);
        let open_path = self.convert_fspath_to_abs(fs_path)?;
        let inode = if creation_flags.no_follow_symlink() {
            match self.lookup_inode_no_follow(fs_path).await {
                Ok(inode) => {
                    let status_flags = StatusFlags::from_bits_truncate(flags);
                    if inode.metadata().await?.type_ == FileType::SymLink
                        && !status_flags.is_fast_open()
                    {
                        return_errno!(ELOOP, "file is a symlink");
                    }
                    if creation_flags.can_create() && creation_flags.is_exclusive() {
                        return_errno!(EEXIST, "file exists");
                    }
                    if creation_flags.must_be_directory()
                        && inode.metadata().await?.type_ != FileType::Dir
                    {
                        return_errno!(
                            ENOTDIR,
                            "O_DIRECTORY is specified but file is not a directory"
                        );
                    }
                    inode
                }
                Err(e) if e.errno() == ENOENT && creation_flags.can_create() => {
                    if creation_flags.must_be_directory() {
                        return_errno!(ENOTDIR, "file is not directory");
                    }
                    if fs_path.ends_with("/") {
                        return_errno!(EISDIR, "path is a directory");
                    }
                    let (dir_inode, file_name) = self.lookup_dirinode_and_basename(fs_path).await?;
                    if !dir_inode.allow_write() {
                        return_errno!(EPERM, "file cannot be created");
                    }
                    dir_inode
                        .create(&file_name, FileType::File, mode.bits())
                        .await?
                }
                Err(e) => return Err(e),
            }
        } else {
            match self.lookup_inode(fs_path).await {
                Ok(inode) => {
                    if creation_flags.can_create() && creation_flags.is_exclusive() {
                        return_errno!(EEXIST, "file exists");
                    }
                    if creation_flags.must_be_directory()
                        && inode.metadata().await?.type_ != FileType::Dir
                    {
                        return_errno!(
                            ENOTDIR,
                            "O_DIRECTORY is specified but file is not a directory"
                        );
                    }
                    inode
                }
                Err(e) if e.errno() == ENOENT && creation_flags.can_create() => {
                    if creation_flags.must_be_directory() {
                        return_errno!(ENOTDIR, "file is not directory");
                    }
                    if fs_path.ends_with("/") {
                        return_errno!(EISDIR, "path is a directory");
                    }
                    let (dir_inode, file_name) =
                        self.lookup_real_dirinode_and_basename(fs_path).await?;
                    if file_name.ends_with("/") {
                        return_errno!(EISDIR, "path refers to directory");
                    }
                    if !dir_inode.allow_write() {
                        return_errno!(EPERM, "file cannot be created");
                    }
                    dir_inode
                        .create(&file_name, FileType::File, mode.bits())
                        .await?
                }
                Err(e) => return Err(e),
            }
        };

        Ok(match inode {
            InodeHandle::Sync(inode) => {
                let inode_file = InodeFile::open(inode, flags, open_path)?;
                FileRef::new_inode(inode_file)
            }
            InodeHandle::Async(inode) => {
                let dentry = Dentry::new(inode, open_path);
                let async_file_handle = AsyncFileHandle::open(
                    dentry,
                    AccessMode::from_u32(flags)?,
                    CreationFlags::from_bits_truncate(flags),
                    StatusFlags::from_bits_truncate(flags),
                )
                .await?;
                FileRef::new_async_file_handle(async_file_handle)
            }
        })
    }

    /// Lookup Inode from the fs view of the process.
    /// If last component is a symlink, do not dereference it
    pub async fn lookup_inode_no_follow(&self, fs_path: &FsPath) -> Result<InodeHandle> {
        debug!(
            "lookup_inode_no_follow: cwd: {:?}, path: {:?}",
            self.cwd(),
            fs_path
        );
        self.lookup_inode_inner(fs_path, false).await
    }

    /// Lookup inode from the fs view of the process, dereference symlinks
    pub async fn lookup_inode(&self, fs_path: &FsPath) -> Result<InodeHandle> {
        debug!("lookup_inode: cwd: {:?}, path: {:?}", self.cwd(), fs_path);
        self.lookup_inode_inner(fs_path, true).await
    }

    async fn lookup_inode_inner(
        &self,
        fs_path: &FsPath,
        follow_symlink: bool,
    ) -> Result<InodeHandle> {
        let inode = match fs_path.inner() {
            FsPathInner::Absolute(path) | FsPathInner::CwdRelative(path) => {
                if follow_symlink {
                    self.lookup_inode_cwd(path).await?
                } else {
                    self.lookup_inode_cwd_no_follow(path).await?
                }
            }
            FsPathInner::Cwd => {
                if follow_symlink {
                    self.lookup_inode_cwd(&self.cwd()).await?
                } else {
                    self.lookup_inode_cwd_no_follow(&self.cwd()).await?
                }
            }
            FsPathInner::FdRelative(dirfd, path) => {
                let inode = self.lookup_inode_from_fd(*dirfd)?;
                if follow_symlink {
                    inode.lookup(path, Some(MAX_SYMLINKS)).await?
                } else {
                    if path.ends_with("/") {
                        inode.lookup(path, Some(MAX_SYMLINKS)).await?
                    } else {
                        let (dir_path, base_name) = split_path(path);
                        let dir_inode = inode.lookup(dir_path, Some(MAX_SYMLINKS)).await?;
                        dir_inode.lookup_no_follow(base_name).await?
                    }
                }
            }
            FsPathInner::Fd(fd) => self.lookup_inode_from_fd(*fd)?,
        };

        Ok(inode)
    }

    /// Lookup the dir_inode and basename
    ///
    /// The `basename` is the last component of fs_path. It can be suffixed by "/".
    pub async fn lookup_dirinode_and_basename(
        &self,
        fs_path: &FsPath,
    ) -> Result<(InodeHandle, String)> {
        let (dir_inode, base_name) = match fs_path.inner() {
            FsPathInner::Absolute(path) | FsPathInner::CwdRelative(path) => {
                let (dir_path, base_name) = split_path(path);
                (self.lookup_inode_cwd(dir_path).await?, base_name.to_owned())
            }
            FsPathInner::FdRelative(dirfd, path) => {
                let inode = self.lookup_inode_from_fd(*dirfd)?;
                let (dir_path, base_name) = split_path(path);
                let dir_inode = inode.lookup(dir_path, Some(MAX_SYMLINKS)).await?;
                (dir_inode, base_name.to_owned())
            }
            _ => return_errno!(ENOENT, "cannot find dir and basename with empty path"),
        };
        Ok((dir_inode, base_name))
    }

    /// Lookup the real dir inode and basename.
    /// It is used to create new file in `open_file`.
    async fn lookup_real_dirinode_and_basename(
        &self,
        fs_path: &FsPath,
    ) -> Result<(InodeHandle, String)> {
        let (dir_inode, base_name) = match fs_path.inner() {
            FsPathInner::Absolute(path) | FsPathInner::CwdRelative(path) => {
                let real_path = self.lookup_real_path(None, path).await?;
                let (dir_path, base_name) = split_path(&real_path);
                (self.lookup_inode_cwd(dir_path).await?, base_name.to_owned())
            }
            FsPathInner::FdRelative(dirfd, path) => {
                let inode = self.lookup_inode_from_fd(*dirfd)?;
                let real_path = self.lookup_real_path(Some(&inode), path).await?;
                let (dir_path, base_name) = split_path(&real_path);
                let dir_inode = if dir_path.starts_with("/") {
                    self.lookup_inode_cwd(dir_path).await?
                } else {
                    inode.lookup(dir_path, Some(MAX_SYMLINKS)).await?
                };
                (dir_inode, base_name.to_owned())
            }
            _ => return_errno!(ENOENT, "cannot find real dir and basename with empty path"),
        };
        Ok((dir_inode, base_name))
    }

    /// Lookup the real path of giving path, dereference symlinks.
    /// If parent is provided, it will lookup the real path from the parent inode.
    /// If parent is not provided, it will lookup the real path from the cwd of process.
    async fn lookup_real_path(&self, parent: Option<&InodeHandle>, path: &str) -> Result<String> {
        let mut real_path = String::from(path);

        loop {
            let (dir_path, file_name) = split_path(&real_path);
            let dir_inode = if let Some(parent_inode) = parent {
                parent_inode.lookup(dir_path, Some(MAX_SYMLINKS)).await?
            } else {
                self.lookup_inode_cwd(dir_path).await?
            };

            match dir_inode
                .lookup_no_follow(file_name.trim_end_matches('/'))
                .await
            {
                Ok(inode) if inode.metadata().await?.type_ == FileType::SymLink => {
                    // Update real_path for next round
                    real_path = {
                        let mut new_path = {
                            let link_path = inode.read_link().await?;
                            if link_path.starts_with("/") {
                                link_path
                            } else {
                                String::from(dir_path) + "/" + &link_path
                            }
                        };
                        if real_path.ends_with("/") && !new_path.ends_with("/") {
                            new_path += "/";
                        }
                        new_path
                    };
                }
                Ok(_) => {
                    debug!("real path: {:?}", real_path);
                    return Ok(real_path);
                }
                Err(e) if e.errno() == ENOENT => {
                    debug!("real path: {:?}", real_path);
                    return Ok(real_path);
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Lookup Inode from the cwd of the process. If path is a symlink, do not dereference it
    async fn lookup_inode_cwd_no_follow(&self, path: &str) -> Result<InodeHandle> {
        if path.ends_with("/") {
            Ok(self.lookup_inode_cwd(path).await?)
        } else {
            let (dir_path, file_name) = split_path(&path);
            let dir_inode = self.lookup_inode_cwd(dir_path).await?;
            Ok(dir_inode.lookup_no_follow(file_name).await?)
        }
    }

    async fn lookup_inode_cwd(&self, path: &str) -> Result<InodeHandle> {
        let full_path_string = if path.starts_with("/") {
            // absolute path
            path.to_owned()
        } else {
            // relative path
            let cwd = self.cwd();
            if !cwd.ends_with("/") {
                cwd + "/" + path
            } else {
                cwd + path
            }
        };
        let full_path = full_path_string.trim_start_matches('/');

        let inode = if full_path.starts_with(ASYNC_SFS_NAME) {
            let path = full_path.strip_prefix(ASYNC_SFS_NAME).unwrap();
            let inode = async_sfs()
                .await
                .root_inode()
                .await
                .lookup_follow(path, Some(MAX_SYMLINKS))
                .await?;
            InodeHandle::from_async(inode)
        } else {
            let inode = ROOT_FS
                .read()
                .unwrap()
                .root_inode()
                .lookup_follow(full_path, MAX_SYMLINKS)?;
            InodeHandle::from_sync(inode)
        };
        Ok(inode)
    }

    fn lookup_inode_from_fd(&self, fd: FileDesc) -> Result<InodeHandle> {
        let file_ref = current!().file(fd)?;
        let inode = if let Some(inode_file) = file_ref.as_inode_file() {
            let inode = Arc::clone(inode_file.inode());
            InodeHandle::from_sync(inode)
        } else if let Some(async_file_handle) = file_ref.as_async_file_handle() {
            let inode = Arc::clone(async_file_handle.dentry().inode());
            InodeHandle::from_async(inode)
        } else {
            return_errno!(EBADF, "dirfd is not an inode file");
        };
        Ok(inode)
    }

    /// Convert the FsPath to the absolute path.
    /// This function is used to record the open path for a file.
    ///
    /// TODO: Introducing dentry cache to get the full path from inode.
    pub fn convert_fspath_to_abs(&self, fs_path: &FsPath) -> Result<String> {
        let abs_path = match fs_path.inner() {
            FsPathInner::Absolute(path) => (*path).to_owned(),
            FsPathInner::CwdRelative(path) => {
                let cwd = self.cwd();
                if !cwd.ends_with("/") {
                    cwd + "/" + path
                } else {
                    cwd + path
                }
            }
            FsPathInner::FdRelative(dirfd, path) => {
                let file_ref = current!().file(*dirfd)?;
                let dir_path = if let Some(inode_file) = file_ref.as_inode_file() {
                    inode_file.open_path().to_owned()
                } else if let Some(async_file_handle) = file_ref.as_async_file_handle() {
                    async_file_handle.dentry().abs_path().to_owned()
                } else {
                    return_errno!(EBADF, "dirfd is not an inode file");
                };
                if !dir_path.ends_with("/") {
                    dir_path + "/" + path
                } else {
                    dir_path + path
                }
            }
            FsPathInner::Cwd => self.cwd(),
            FsPathInner::Fd(fd) => {
                let file_ref = current!().file(*fd)?;
                if let Some(inode_file) = file_ref.as_inode_file() {
                    inode_file.open_path().to_owned()
                } else if let Some(async_file_handle) = file_ref.as_async_file_handle() {
                    async_file_handle.dentry().abs_path().to_owned()
                } else {
                    return_errno!(EBADF, "dirfd is not an inode file");
                }
            }
        };

        if abs_path.len() > PATH_MAX {
            return_errno!(ENAMETOOLONG, "abs path too long");
        }

        Ok(abs_path)
    }
}

impl Default for FsView {
    fn default() -> Self {
        Self::new()
    }
}

/// Split a `path` to (`dir_path`, `file_name`).
///
/// The `dir_path` must be a directory.
///
/// The `file_name` is the last component. It can be suffixed by "/".
///
/// Example:
///
/// The path "/dir/file/" will be split to ("/dir", "file/").
pub fn split_path(path: &str) -> (&str, &str) {
    let file_name = path
        .split_inclusive('/')
        .filter(|&x| x != "/")
        .last()
        .unwrap_or(".");

    let mut split = path.trim_end_matches('/').rsplitn(2, '/');
    let dir_path = if split.next().unwrap().is_empty() {
        "/"
    } else {
        let mut dir = split.next().unwrap_or(".").trim_end_matches('/');
        if dir.is_empty() {
            dir = "/";
        }
        dir
    };

    (dir_path, file_name)
}

// Linux uses 40 as the upper limit for resolving symbolic links,
// so Occlum use it as a reasonable value
pub const MAX_SYMLINKS: usize = 40;
