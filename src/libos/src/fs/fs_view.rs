/// Present a per-process view of FS.
use super::*;

#[derive(Debug, Clone)]
pub struct FsView {
    root: String,
    cwd: String,
}

impl FsView {
    pub fn new() -> FsView {
        let root = String::from("/");
        let cwd = root.clone();
        Self { root, cwd }
    }

    /// Get the root directory
    pub fn root(&self) -> &str {
        &self.root
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

        if path.as_bytes()[0] == b'/' {
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
    pub fn open_file(&self, path: &str, flags: u32, mode: FileMode) -> Result<Arc<dyn File>> {
        let creation_flags = CreationFlags::from_bits_truncate(flags);
        let inode = if creation_flags.no_follow_symlink() {
            match self.lookup_inode_no_follow(path) {
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
                    let (dir_path, file_name) = split_path(&path);
                    let dir_inode = self.lookup_inode(dir_path)?;
                    if !dir_inode.allow_write()? {
                        return_errno!(EPERM, "file cannot be created");
                    }
                    dir_inode.create(file_name, FileType::File, mode.bits())?
                }
                Err(e) => return Err(e),
            }
        } else {
            match self.lookup_inode(path) {
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
                    let real_path = self.lookup_real_path(&path)?;
                    let (dir_path, file_name) = split_path(&real_path);
                    let dir_inode = self.lookup_inode(dir_path)?;
                    if !dir_inode.allow_write()? {
                        return_errno!(EPERM, "file cannot be created");
                    }
                    dir_inode.create(file_name, FileType::File, mode.bits())?
                }
                Err(e) => return Err(e),
            }
        };
        let abs_path = self.convert_to_abs_path(&path);
        Ok(Arc::new(INodeFile::open(inode, &abs_path, flags)?))
    }

    /// Recursively lookup the real path of giving path, dereference symlinks
    pub fn lookup_real_path(&self, path: &str) -> Result<String> {
        let (dir_path, file_name) = split_path(&path);
        let dir_inode = self.lookup_inode(dir_path)?;
        match dir_inode.find(file_name) {
            // Handle symlink
            Ok(inode) if inode.metadata()?.type_ == FileType::SymLink => {
                let new_path = {
                    let mut content = vec![0u8; PATH_MAX];
                    let len = inode.read_at(0, &mut content)?;
                    let path = std::str::from_utf8(&content[..len])
                        .map_err(|_| errno!(ENOENT, "invalid symlink content"))?;
                    let path = String::from(path);
                    match path.chars().next() {
                        None => unreachable!(),
                        // absolute path
                        Some('/') => path,
                        // relative path
                        _ => {
                            let dir_path = if dir_path.ends_with("/") {
                                String::from(dir_path)
                            } else {
                                String::from(dir_path) + "/"
                            };
                            dir_path + &path
                        }
                    }
                };
                self.lookup_real_path(&new_path)
            }
            Err(FsError::EntryNotFound) | Ok(_) => {
                debug!("real_path: cwd: {:?}, path: {:?}", self.cwd(), path);
                Ok(String::from(path))
            }
            Err(e) => return Err(Error::from(e)),
        }
    }

    /// Lookup INode from the cwd of the process. If path is a symlink, do not dereference it
    pub fn lookup_inode_no_follow(&self, path: &str) -> Result<Arc<dyn INode>> {
        debug!(
            "lookup_inode_no_follow: cwd: {:?}, path: {:?}",
            self.cwd(),
            path
        );
        let (dir_path, file_name) = split_path(&path);
        let dir_inode = self.lookup_inode(dir_path)?;
        Ok(dir_inode.lookup(file_name)?)
    }

    /// Lookup INode from the cwd of the process, dereference symlink
    pub fn lookup_inode(&self, path: &str) -> Result<Arc<dyn INode>> {
        debug!("lookup_inode: cwd: {:?}, path: {:?}", self.cwd(), path);
        if path.len() > 0 && path.as_bytes()[0] == b'/' {
            // absolute path
            let abs_path = path.trim_start_matches('/');
            let inode = ROOT_FS
                .read()
                .unwrap()
                .root_inode()
                .lookup_follow(abs_path, MAX_SYMLINKS)?;
            Ok(inode)
        } else {
            // relative path
            let cwd = self.cwd().trim_start_matches('/');
            let inode = ROOT_FS
                .read()
                .unwrap()
                .root_inode()
                .lookup_follow(cwd, MAX_SYMLINKS)?
                .lookup_follow(path, MAX_SYMLINKS)?;
            Ok(inode)
        }
    }

    /// Convert the path to be absolute
    pub fn convert_to_abs_path(&self, path: &str) -> String {
        debug!(
            "convert_to_abs_path: cwd: {:?}, path: {:?}",
            self.cwd(),
            path
        );
        if path.len() > 0 && path.as_bytes()[0] == b'/' {
            // path is absolute path already
            return path.to_owned();
        }
        let cwd = {
            if !self.cwd().ends_with("/") {
                self.cwd().to_owned() + "/"
            } else {
                self.cwd().to_owned()
            }
        };
        cwd + path
    }
}

impl Default for FsView {
    fn default() -> Self {
        let root = String::from("/");
        let cwd = root.clone();
        Self { root, cwd }
    }
}
