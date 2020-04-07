use super::dev_fs::{DevNull, DevRandom, DevSgx, DevZero};
/// Present a per-process view of FS.
use super::*;

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
    pub fn open_file(&self, path: &str, flags: u32, mode: u32) -> Result<Box<dyn File>> {
        if path == "/dev/null" {
            return Ok(Box::new(DevNull));
        }
        if path == "/dev/zero" {
            return Ok(Box::new(DevZero));
        }
        if path == "/dev/random" || path == "/dev/urandom" || path == "/dev/arandom" {
            return Ok(Box::new(DevRandom));
        }
        if path == "/dev/sgx" {
            return Ok(Box::new(DevSgx));
        }
        let creation_flags = CreationFlags::from_bits_truncate(flags);
        let inode = if creation_flags.can_create() {
            let (dir_path, file_name) = split_path(&path);
            let dir_inode = self.lookup_inode(dir_path)?;
            match dir_inode.find(file_name) {
                Ok(file_inode) => {
                    if creation_flags.is_exclusive() {
                        return_errno!(EEXIST, "file exists");
                    }
                    file_inode
                }
                Err(FsError::EntryNotFound) => {
                    if !dir_inode.allow_write()? {
                        return_errno!(EPERM, "file cannot be created");
                    }
                    dir_inode.create(file_name, FileType::File, mode)?
                }
                Err(e) => return Err(Error::from(e)),
            }
        } else {
            self.lookup_inode(&path)?
        };
        let abs_path = self.convert_to_abs_path(&path);
        Ok(Box::new(INodeFile::open(inode, &abs_path, flags)?))
    }

    /// Lookup INode from the cwd of the process
    pub fn lookup_inode(&self, path: &str) -> Result<Arc<dyn INode>> {
        debug!("lookup_inode: cwd: {:?}, path: {:?}", self.cwd(), path);
        if path.len() > 0 && path.as_bytes()[0] == b'/' {
            // absolute path
            let abs_path = path.trim_start_matches('/');
            let inode = ROOT_INODE.lookup(abs_path)?;
            Ok(inode)
        } else {
            // relative path
            let cwd = self.cwd().trim_start_matches('/');
            let inode = ROOT_INODE.lookup(cwd)?.lookup(path)?;
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
        Self {
            cwd: "/".to_owned(),
        }
    }
}
