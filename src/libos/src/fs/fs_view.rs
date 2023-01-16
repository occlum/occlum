/// Present a per-process view of FS.
use super::*;

use super::fspath::FsPathInner;

#[derive(Debug)]
pub struct FsView {
    inner: RwLock<Inner>,
}

#[derive(Debug)]
enum Inner {
    View { root: Arc<Dentry>, cwd: Arc<Dentry> },
    Dummy,
}

impl Inner {
    pub fn is_dummy(&self) -> bool {
        match self {
            Self::Dummy => true,
            _ => false,
        }
    }

    pub fn root(&self) -> Arc<Dentry> {
        match self {
            Self::View { root, .. } => root.clone(),
            Self::Dummy => panic!("dummy fs view"),
        }
    }

    pub fn cwd(&self) -> Arc<Dentry> {
        match self {
            Self::View { cwd, .. } => cwd.clone(),
            Self::Dummy => panic!("dummy fs view"),
        }
    }

    pub fn set_root(&mut self, new_root: Arc<Dentry>) {
        match self {
            Self::View { root, cwd } => {
                *root = new_root.clone();
                *cwd = new_root;
            }
            Self::Dummy => {
                *self = Self::View {
                    root: new_root.clone(),
                    cwd: new_root,
                };
            }
        }
    }

    pub fn set_cwd(&mut self, new_cwd: Arc<Dentry>) {
        match self {
            Self::View { cwd, .. } => {
                *cwd = new_cwd;
            }
            Self::Dummy => panic!("dummy fs view"),
        }
    }
}

impl Clone for FsView {
    fn clone(&self) -> Self {
        Self {
            inner: RwLock::new(Inner::View {
                root: self.root(),
                cwd: self.cwd(),
            }),
        }
    }
}

impl FsView {
    /// Can be used for the idle process only.
    pub fn dummy() -> FsView {
        Self {
            inner: RwLock::new(Inner::Dummy),
        }
    }

    /// Check if is dummy.
    pub fn is_dummy(&self) -> bool {
        self.inner.read().unwrap().is_dummy()
    }

    /// Create a FsView with rootfs's root inode as the root.
    pub async fn new() -> FsView {
        let root = Dentry::new_root(rootfs().await.root_inode().await);
        let cwd = root.clone();
        Self {
            inner: RwLock::new(Inner::View { root, cwd }),
        }
    }

    /// Get the root dentry.
    pub fn root(&self) -> Arc<Dentry> {
        self.inner.read().unwrap().root()
    }

    /// Set the root dentry, this operation will also change the cwd.
    pub fn set_root(&self, new_root: Arc<Dentry>) {
        self.inner.write().unwrap().set_root(new_root)
    }

    /// Get the current working dentry.
    pub fn cwd(&self) -> Arc<Dentry> {
        self.inner.read().unwrap().cwd()
    }

    /// Set the current working dentry.
    pub fn set_cwd(&self, new_cwd: Arc<Dentry>) {
        self.inner.write().unwrap().set_cwd(new_cwd)
    }

    /// Open a file on the process. But DO NOT add it to file table.
    pub async fn open_file(&self, fs_path: &FsPath, flags: u32, mode: FileMode) -> Result<FileRef> {
        let creation_flags = CreationFlags::from_bits_truncate(flags);
        let status_flags = StatusFlags::from_bits_truncate(flags);
        let access_mode = AccessMode::from_u32(flags)?;
        let follow_tail_symlink = !creation_flags.no_follow_symlink();

        let dentry = match self.lookup_inner(fs_path, follow_tail_symlink).await {
            Ok(dentry) => {
                let inode = dentry.inode();
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
                dentry
            }
            Err(e) if e.errno() == ENOENT && creation_flags.can_create() => {
                if creation_flags.must_be_directory() {
                    return_errno!(ENOTDIR, "cannot create directory");
                }
                let (dir_dentry, file_name) = self
                    .lookup_dir_and_base_name_inner(fs_path, follow_tail_symlink)
                    .await?;
                if file_name.ends_with("/") {
                    return_errno!(EISDIR, "path is a directory");
                }
                if !dir_dentry.inode().allow_write().await {
                    return_errno!(EPERM, "file cannot be created");
                }
                let new_dentry = dir_dentry.create(&file_name, FileType::File, mode).await?;
                new_dentry
            }
            Err(e) => return Err(e),
        };

        let file_ref = {
            let async_file_handle =
                AsyncFileHandle::open(dentry, access_mode, creation_flags, status_flags).await?;
            FileRef::new_async_file_handle(async_file_handle)
        };
        Ok(file_ref)
    }

    /// Lookup dentry from the fs view of the process.
    ///
    /// Do not follow it if last component is a symlink
    pub async fn lookup_no_follow(&self, fs_path: &FsPath) -> Result<Arc<Dentry>> {
        self.lookup_inner(fs_path, false).await
    }

    /// Lookup dentry from the fs view of the process.
    ///
    /// Follow symlinks
    pub async fn lookup(&self, fs_path: &FsPath) -> Result<Arc<Dentry>> {
        self.lookup_inner(fs_path, true).await
    }

    async fn lookup_inner(
        &self,
        fs_path: &FsPath,
        follow_tail_symlink: bool,
    ) -> Result<Arc<Dentry>> {
        debug!(
            "lookup_inner: path: {:?}, follow_tail_symlink: {}",
            fs_path, follow_tail_symlink
        );
        let dentry = match fs_path.inner() {
            FsPathInner::Absolute(path) => {
                self.lookup_from_parent(
                    self.root(),
                    path.trim_start_matches('/'),
                    follow_tail_symlink,
                )
                .await?
            }
            FsPathInner::CwdRelative(path) => {
                self.lookup_from_parent(self.cwd(), path, follow_tail_symlink)
                    .await?
            }
            FsPathInner::Cwd => self.cwd(),
            FsPathInner::FdRelative(dirfd, path) => {
                let dir_dentry = self.lookup_from_fd(*dirfd)?;
                self.lookup_from_parent(dir_dentry, path, follow_tail_symlink)
                    .await?
            }
            FsPathInner::Fd(fd) => self.lookup_from_fd(*fd)?,
        };
        Ok(dentry)
    }

    /// Lookup the dir dentry and base file name of the giving path.
    ///
    /// If the last component is a symlink, do not deference it
    pub async fn lookup_dir_and_base_name(
        &self,
        fs_path: &FsPath,
    ) -> Result<(Arc<Dentry>, String)> {
        self.lookup_dir_and_base_name_inner(fs_path, false).await
    }

    /// Lookup the dir dentry and base file name of the giving path.
    ///
    /// If the last component is a symlink, should deference it
    pub async fn lookup_dir_and_base_name_follow(
        &self,
        fs_path: &FsPath,
    ) -> Result<(Arc<Dentry>, String)> {
        self.lookup_dir_and_base_name_inner(fs_path, true).await
    }

    async fn lookup_dir_and_base_name_inner(
        &self,
        fs_path: &FsPath,
        follow_tail_symlink: bool,
    ) -> Result<(Arc<Dentry>, String)> {
        debug!(
            "lookup_dir_and_base_name_inner: fs_path: {:?}, follow_tail_symlink: {}",
            fs_path, follow_tail_symlink
        );
        // Initialize the first dir dentry and the base name
        let (mut dir_dentry, mut base_name) = match fs_path.inner() {
            FsPathInner::Absolute(path) => {
                let (dir, file_name) = split_path(path);
                let dir_dentry = self
                    .lookup_from_parent(self.root(), dir.trim_start_matches('/'), true)
                    .await?;
                (dir_dentry, file_name.to_owned())
            }
            FsPathInner::CwdRelative(path) => {
                let (dir, file_name) = split_path(path);
                (
                    self.lookup_from_parent(self.cwd(), dir, true).await?,
                    file_name.to_owned(),
                )
            }
            FsPathInner::FdRelative(dirfd, path) => {
                let (dir, file_name) = split_path(path);
                let parent = self.lookup_from_fd(*dirfd)?;
                (
                    self.lookup_from_parent(parent, dir, true).await?,
                    file_name.to_owned(),
                )
            }
            _ => return_errno!(ENOENT, "cannot find dir and basename with empty path"),
        };

        if !follow_tail_symlink {
            return Ok((dir_dentry, base_name));
        }

        // Deference symlinks
        loop {
            match dir_dentry.find(&base_name.trim_end_matches('/')).await {
                Ok(dentry) if dentry.inode().metadata().await?.type_ == FileType::SymLink => {
                    let link = {
                        let mut link = dentry.inode().read_link().await?;
                        if link.is_empty() {
                            return_errno!(ENOENT, "invalid symlink");
                        }
                        if base_name.ends_with("/") && !link.ends_with("/") {
                            link += "/";
                        }
                        link
                    };
                    let (dir, file_name) = split_path(&link);
                    if dir.starts_with("/") {
                        dir_dentry = self
                            .lookup_from_parent(self.root(), dir.trim_start_matches('/'), true)
                            .await?;
                        base_name = file_name.to_owned();
                    } else {
                        dir_dentry = self.lookup_from_parent(dir_dentry, dir, true).await?;
                        base_name = file_name.to_owned();
                    }
                }
                _ => break,
            }
        }

        Ok((dir_dentry, base_name))
    }

    /// Lookup dentry from parent.
    ///
    /// The length of `path` cannot exceed PATH_MAX.
    /// If `path` ends with `/`, then the returned inode must be a directory inode.
    ///
    /// While looking up the dentry, symbolic links will be followed for
    /// at most `MAX_SYMLINKS` times.
    ///
    /// If `follow_tail_link` is true and the trailing component is a symlink,
    /// it will be followed.
    /// Symlinks in earlier components of the path will always be followed.
    async fn lookup_from_parent(
        &self,
        parent: Arc<Dentry>,
        relative_path: &str,
        follow_tail_symlink: bool,
    ) -> Result<Arc<Dentry>> {
        debug_assert!(!relative_path.starts_with("/"));
        if relative_path.len() > PATH_MAX {
            return_errno!(ENAMETOOLONG, "path is too long");
        }

        // To handle symlinks
        let mut link_path = String::new();
        let mut follows = 0;

        // Initialize the first dentry and the relative path
        let (mut dentry, mut relative_path) = (parent, relative_path);

        while !relative_path.is_empty() {
            let (next_name, path_remain, must_be_dir) =
                if let Some((prefix, suffix)) = relative_path.split_once('/') {
                    let suffix = suffix.trim_start_matches('/');
                    (prefix, suffix, true)
                } else {
                    (relative_path, "", false)
                };

            // Iterate next dentry
            let next_dentry = dentry.find(next_name).await?;
            let next_type = next_dentry.inode().metadata().await?.type_;
            let next_is_tail = path_remain.is_empty();

            // If next type is a symlink, follow symlinks at most `MAX_SYMLINKS` times.
            if next_type == FileType::SymLink && (follow_tail_symlink || !next_is_tail) {
                if follows >= MAX_SYMLINKS {
                    return_errno!(ELOOP, "too many symlinks");
                }
                let link_path_remain = {
                    let mut tmp_link_path = next_dentry.inode().read_link().await?;
                    if tmp_link_path.is_empty() {
                        return_errno!(ENOENT, "empty symlink");
                    }
                    if !path_remain.is_empty() {
                        tmp_link_path += "/";
                        tmp_link_path += path_remain;
                    } else if must_be_dir {
                        tmp_link_path += "/";
                    }
                    tmp_link_path
                };

                // Change the dentry and relative path according to symlink
                if link_path_remain.starts_with("/") {
                    dentry = self.root();
                }
                link_path.clear();
                link_path.push_str(&link_path_remain.trim_start_matches('/'));
                relative_path = &link_path;
                follows += 1;
            } else {
                // If path ends with `/`, the inode must be a directory
                if must_be_dir && next_type != FileType::Dir {
                    return_errno!(ENOTDIR, "inode is not dir");
                }
                dentry = next_dentry;
                relative_path = path_remain;
            }
        }

        Ok(dentry)
    }

    /// Lookup dentry from the giving fd.
    fn lookup_from_fd(&self, fd: FileDesc) -> Result<Arc<Dentry>> {
        let file_ref = current!().file(fd)?;
        let dentry = if let Some(async_file_handle) = file_ref.as_async_file_handle() {
            Arc::clone(async_file_handle.dentry())
        } else {
            return_errno!(EBADF, "dirfd is not an inode file");
        };
        Ok(dentry)
    }

    /// Convert the FsPath to the absolute path.
    pub fn convert_fspath_to_abs(&self, fs_path: &FsPath) -> Result<String> {
        let abs_path = match fs_path.inner() {
            FsPathInner::Absolute(path) => (*path).to_owned(),
            FsPathInner::CwdRelative(path) => {
                let cwd_path = self.cwd().abs_path();
                cwd_path + "/" + path
            }
            FsPathInner::FdRelative(dirfd, path) => {
                let dir_dentry = self.lookup_from_fd(*dirfd)?;
                let dir_path = dir_dentry.abs_path();
                dir_path + "/" + path
            }
            FsPathInner::Cwd => self.cwd().abs_path(),
            FsPathInner::Fd(fd) => {
                let dentry = self.lookup_from_fd(*fd)?;
                dentry.abs_path()
            }
        };

        if abs_path.len() > PATH_MAX {
            return_errno!(ENAMETOOLONG, "abs path too long");
        }

        Ok(abs_path)
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
