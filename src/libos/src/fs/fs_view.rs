/// Present a per-process view of FS.
use super::*;

use super::fspath::FsPathInner;

#[derive(Debug, Clone)]
pub struct FsView {
    cwd: String,
}

impl FsView {
    pub fn new() -> FsView {
        Self {
            cwd: "/".to_owned(),
        }
    }

    /// Get the current working directory.
    pub fn cwd(&self) -> &str {
        &self.cwd
    }

    /// Set the current working directory.
    pub fn set_cwd(&mut self, path: &str) -> Result<()> {
        if path.len() == 0 {
            return_errno!(EINVAL, "empty path");
        }

        if let Some('/') = path.chars().next() {
            // absolute
            self.cwd = path.to_owned();
        } else {
            // relative
            if !self.cwd.ends_with("/") {
                self.cwd += "/";
            }
            self.cwd += path;
        }
        Ok(())
    }

    /// Open a file on the process. But DO NOT add it to file table.
    pub fn open_file(&self, fs_path: &FsPath, flags: u32, mode: u32) -> Result<INodeFile> {
        let creation_flags = CreationFlags::from_bits_truncate(flags);
        let inode = if creation_flags.no_follow_symlink() {
            match self.lookup_inode_no_follow(fs_path) {
                Ok(inode) => {
                    let status_flags = StatusFlags::from_bits_truncate(flags);
                    if inode.metadata()?.type_ == FileType::SymLink && !status_flags.is_fast_open()
                    {
                        return_errno!(ELOOP, "file is a symlink");
                    }
                    if creation_flags.can_create() && creation_flags.is_exclusive() {
                        return_errno!(EEXIST, "file exists");
                    }
                    if creation_flags.must_be_directory()
                        && inode.metadata()?.type_ != FileType::Dir
                    {
                        return_errno!(
                            ENOTDIR,
                            "O_DIRECTORY is specified but file is not a directory"
                        );
                    }
                    inode
                }
                Err(e) if e.errno() == ENOENT && creation_flags.can_create() => {
                    let (dir_inode, file_name) = self.lookup_dirinode_and_basename(fs_path)?;
                    if !dir_inode.allow_write()? {
                        return_errno!(EPERM, "file cannot be created");
                    }
                    dir_inode.create(&file_name, FileType::File, mode)?
                }
                Err(e) => return Err(e),
            }
        } else {
            match self.lookup_inode(fs_path) {
                Ok(inode) => {
                    if creation_flags.can_create() && creation_flags.is_exclusive() {
                        return_errno!(EEXIST, "file exists");
                    }
                    if creation_flags.must_be_directory()
                        && inode.metadata()?.type_ != FileType::Dir
                    {
                        return_errno!(
                            ENOTDIR,
                            "O_DIRECTORY is specified but file is not a directory"
                        );
                    }
                    inode
                }
                Err(e) if e.errno() == ENOENT && creation_flags.can_create() => {
                    let (dir_inode, file_name) = self.lookup_real_dirinode_and_basename(fs_path)?;
                    if !dir_inode.allow_write()? {
                        return_errno!(EPERM, "file cannot be created");
                    }
                    dir_inode.create(&file_name, FileType::File, mode)?
                }
                Err(e) => return Err(e),
            }
        };
        Ok(INodeFile::open(inode, flags)?)
    }

    /// Lookup INode, dereference symlink
    pub fn lookup_inode(&self, fs_path: &FsPath) -> Result<Arc<dyn INode>> {
        debug!("lookup_inode: cwd: {:?}, path: {:?}", self.cwd(), fs_path);
        self.lookup_inode_inner(fs_path, true)
    }

    /// Lookup INode, do not dereference the last symlink component
    pub fn lookup_inode_no_follow(&self, fs_path: &FsPath) -> Result<Arc<dyn INode>> {
        debug!(
            "lookup_inode_no_follow: cwd: {:?}, path: {:?}",
            self.cwd(),
            fs_path
        );
        self.lookup_inode_inner(fs_path, false)
    }

    fn lookup_inode_inner(&self, fs_path: &FsPath, follow_symlink: bool) -> Result<Arc<dyn INode>> {
        let inode = match fs_path.inner() {
            FsPathInner::Absolute(path) | FsPathInner::CwdRelative(path) => {
                if follow_symlink {
                    self.lookup_inode_cwd(path)?
                } else {
                    self.lookup_inode_cwd_no_follow(path)?
                }
            }
            FsPathInner::Cwd => {
                if follow_symlink {
                    self.lookup_inode_cwd(self.cwd())?
                } else {
                    self.lookup_inode_cwd_no_follow(self.cwd())?
                }
            }
            FsPathInner::FdRelative(fd, path) => {
                let inode = self.lookup_inode_from_fd(*fd)?;
                if inode.metadata()?.type_ != FileType::Dir {
                    return_errno!(ENOTDIR, "dirfd is not a directory");
                }
                if follow_symlink {
                    inode.lookup_follow(path, MAX_SYMLINKS)?
                } else {
                    let (dir_path, base_name) = split_path(path);
                    let dir_inode = inode.lookup_follow(dir_path, MAX_SYMLINKS)?;
                    dir_inode.lookup(base_name)?
                }
            }
            FsPathInner::Fd(fd) => self.lookup_inode_from_fd(*fd)?,
        };

        Ok(inode)
    }

    /// Lookup dir inode and basename
    pub fn lookup_dirinode_and_basename(
        &self,
        fs_path: &FsPath,
    ) -> Result<(Arc<dyn INode>, String)> {
        let (dir_inode, base_name) = match fs_path.inner() {
            FsPathInner::Absolute(path) | FsPathInner::CwdRelative(path) => {
                let (dir_path, base_name) = split_path(path);
                (self.lookup_inode_cwd(dir_path)?, base_name.to_owned())
            }
            FsPathInner::FdRelative(fd, path) => {
                let inode = self.lookup_inode_from_fd(*fd)?;
                let (dir_path, base_name) = split_path(path);
                let dir_inode = inode.lookup_follow(dir_path, MAX_SYMLINKS)?;
                (dir_inode, base_name.to_owned())
            }
            _ => return_errno!(ENOENT, "cannot find dir and basename with empty path"),
        };
        Ok((dir_inode, base_name))
    }

    /// Lookup the real dir inode and basename.
    /// It is used to create new file in `open_file`.
    fn lookup_real_dirinode_and_basename(
        &self,
        fs_path: &FsPath,
    ) -> Result<(Arc<dyn INode>, String)> {
        let (dir_inode, base_name) = match fs_path.inner() {
            FsPathInner::Absolute(path) | FsPathInner::CwdRelative(path) => {
                let real_path = self.lookup_real_path(None, path)?;
                let (dir_path, base_name) = split_path(&real_path);
                (self.lookup_inode_cwd(dir_path)?, base_name.to_owned())
            }
            FsPathInner::FdRelative(fd, path) => {
                let inode = self.lookup_inode_from_fd(*fd)?;
                let real_path = self.lookup_real_path(Some(&inode), path)?;
                let (dir_path, base_name) = split_path(&real_path);
                let dir_inode = if let Some('/') = dir_path.chars().next() {
                    self.lookup_inode_cwd(dir_path)?
                } else {
                    inode.lookup_follow(dir_path, MAX_SYMLINKS)?
                };
                (dir_inode, base_name.to_owned())
            }
            _ => return_errno!(ENOENT, "cannot find real dir and basename with empty path"),
        };
        Ok((dir_inode, base_name))
    }

    /// Lookup INode from the cwd of the process. If path is a symlink, do not dereference it
    fn lookup_inode_cwd_no_follow(&self, path: &str) -> Result<Arc<dyn INode>> {
        let (dir_path, file_name) = split_path(&path);
        let dir_inode = self.lookup_inode_cwd(dir_path)?;
        Ok(dir_inode.lookup(file_name)?)
    }

    /// Lookup INode from the cwd of the process, dereference symlink
    fn lookup_inode_cwd(&self, path: &str) -> Result<Arc<dyn INode>> {
        if let Some('/') = path.chars().next() {
            // absolute path
            let abs_path = path.trim_start_matches('/');
            let inode = ROOT_INODE
                .read()
                .unwrap()
                .lookup_follow(abs_path, MAX_SYMLINKS)?;
            Ok(inode)
        } else {
            // relative path
            let cwd = self.cwd().trim_start_matches('/');
            let inode = ROOT_INODE
                .read()
                .unwrap()
                .lookup_follow(cwd, MAX_SYMLINKS)?
                .lookup_follow(path, MAX_SYMLINKS)?;
            Ok(inode)
        }
    }

    fn lookup_inode_from_fd(&self, fd: FileDesc) -> Result<Arc<dyn INode>> {
        let file_ref = current!().file(fd)?;
        let inode_file = file_ref
            .as_inode_file()
            .ok_or_else(|| errno!(EBADF, "dirfd is not an inode file"))?;
        Ok(Arc::clone(inode_file.inode()))
    }

    /// Recursively lookup the real path of giving path, dereference symlinks.
    /// If parent is provided, it will lookup the real path from the parent inode.
    /// If parent is not provided, it will lookup the real path from the cwd of process.
    fn lookup_real_path(&self, parent: Option<&Arc<dyn INode>>, path: &str) -> Result<String> {
        let (dir_path, file_name) = split_path(&path);
        let dir_inode = if let Some(parent_inode) = parent {
            if let Some('/') = path.chars().next() {
                self.lookup_inode_cwd(dir_path)?
            } else {
                // relative path from parent inode
                parent_inode.lookup_follow(dir_path, MAX_SYMLINKS)?
            }
        } else {
            self.lookup_inode_cwd(dir_path)?
        };

        match dir_inode.lookup(file_name) {
            // Handle symlink
            Ok(inode) if inode.metadata()?.type_ == FileType::SymLink => {
                let new_path = {
                    let path = {
                        let mut content = vec![0u8; PATH_MAX];
                        let len = inode.read_at(0, &mut content)?;
                        let path = std::str::from_utf8(&content[..len])
                            .map_err(|_| errno!(ENOENT, "invalid symlink content"))?;
                        String::from(path)
                    };
                    match path.chars().next() {
                        // absolute path
                        Some('/') => path,
                        // relative path
                        Some(_) => {
                            let dir_path = if dir_path.ends_with("/") {
                                String::from(dir_path)
                            } else {
                                String::from(dir_path) + "/"
                            };
                            dir_path + &path
                        }
                        None => unreachable!(),
                    }
                };
                self.lookup_real_path(parent, &new_path)
            }
            Err(FsError::EntryNotFound) | Ok(_) => {
                debug!("real_path: {:?}", path);
                Ok(String::from(path))
            }
            Err(e) => return Err(Error::from(e)),
        }
    }
}

impl Default for FsView {
    fn default() -> Self {
        Self {
            cwd: "/".to_owned(),
        }
    }
}

// Linux uses 40 as the upper limit for resolving symbolic links,
// so Occlum use it as a reasonable value
const MAX_SYMLINKS: usize = 40;
