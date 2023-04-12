use crate::fs::AsyncFileSystem;
use crate::prelude::*;

use async_io::event::{Events, Poller};
use async_io::fs::{DirentWriterContext, Extension, FallocateMode, FileType, Metadata, PATH_MAX};
use async_io::ioctl::IoctlCmd;
use async_trait::async_trait;
use std::any::Any;

/// Abstract Async Inode object such as file or directory.
#[async_trait]
pub trait AsyncInode: Any + Sync + Send {
    /// Read bytes at `offset` into `buf`, return the number of bytes read.
    async fn read_at(&self, offset: usize, buf: &mut [u8]) -> Result<usize>;

    /// Write bytes at `offset` from `buf`, return the number of bytes written.
    async fn write_at(&self, offset: usize, buf: &[u8]) -> Result<usize>;

    /// Get metadata of the inode
    async fn metadata(&self) -> Result<Metadata>;

    /// Set metadata of the inode
    async fn set_metadata(&self, metadata: &Metadata) -> Result<()>;

    /// Sync all data and metadata
    async fn sync_all(&self) -> Result<()> {
        Ok(())
    }

    /// Sync data (not include metadata)
    async fn sync_data(&self) -> Result<()> {
        Ok(())
    }

    /// Resize file
    async fn resize(&self, _len: usize) -> Result<()> {
        return_errno!(EISDIR, "not file");
    }

    /// Manipulate inode space
    async fn fallocate(&self, _mode: &FallocateMode, _offset: usize, _len: usize) -> Result<()> {
        return_errno!(EOPNOTSUPP, "not support");
    }

    /// Create a new inode in the directory
    async fn create(
        &self,
        _name: &str,
        _type_: FileType,
        _mode: u16,
    ) -> Result<Arc<dyn AsyncInode>> {
        return_errno!(ENOTDIR, "self is not dir");
    }

    /// Create a hard link `name` to `other`
    async fn link(&self, _name: &str, _other: &Arc<dyn AsyncInode>) -> Result<()> {
        return_errno!(ENOTDIR, "self is not dir");
    }

    /// Delete a hard link `name`
    async fn unlink(&self, _name: &str) -> Result<()> {
        return_errno!(ENOTDIR, "self is not dir");
    }

    /// Move inode `self/old_name` to `target/new_name`.
    /// If `target` equals `self`, do rename.
    async fn move_(
        &self,
        _old_name: &str,
        _target: &Arc<dyn AsyncInode>,
        _new_name: &str,
    ) -> Result<()> {
        return_errno!(ENOTDIR, "self is not dir");
    }

    /// Find the inode `name` in the directory
    async fn find(&self, _name: &str) -> Result<Arc<dyn AsyncInode>> {
        return_errno!(ENOTDIR, "self is not dir");
    }

    /// Read the content of symlink
    async fn read_link(&self) -> Result<String> {
        if self.metadata().await?.type_ != FileType::SymLink {
            return_errno!(EINVAL, "not symlink");
        }
        let mut content = vec![0u8; PATH_MAX];
        let len = self.read_at(0, &mut content).await?;
        let path = std::str::from_utf8(&content[..len])
            .map_err(|_| errno!(ENOENT, "invalid symlink content"))?;
        Ok(String::from(path))
    }

    /// Write the content of symlink
    async fn write_link(&self, target: &str) -> Result<()> {
        if self.metadata().await?.type_ != FileType::SymLink {
            return_errno!(EINVAL, "not symlink");
        }
        let data = target.as_bytes();
        self.write_at(0, data).await?;
        Ok(())
    }

    /// Read all contents into a Vec
    async fn read_as_vec(&self) -> Result<Vec<u8>> {
        let size = self.metadata().await?.size;
        let mut buf = Vec::with_capacity(size);
        buf.spare_capacity_mut();
        unsafe {
            buf.set_len(size);
        }
        self.read_at(0, buf.as_mut_slice()).await?;
        Ok(buf)
    }

    /// Get all directory entry name as a Vec
    async fn list(&self) -> Result<Vec<String>> {
        let mut entries = Vec::new();
        let mut dir_ctx = DirentWriterContext::new(0, &mut entries);
        let _ = self.iterate_entries(&mut dir_ctx).await?;
        Ok(entries)
    }

    /// Iterate directory entries
    async fn iterate_entries(&self, _ctx: &mut DirentWriterContext) -> Result<usize> {
        return_errno!(ENOTDIR, "self is not dir");
    }

    /// Control device
    async fn ioctl(&self, _cmd: &mut dyn IoctlCmd) -> Result<()> {
        return_errno!(ENOSYS, "not support");
    }

    /// Mount filesystem at this inode
    async fn mount(&self, _fs: Arc<dyn AsyncFileSystem>) -> Result<()> {
        return_errno!(ENOTDIR, "self is not dir");
    }

    /// Unmount the filesystem which is mounted at this inode
    async fn umount(&self) -> Result<()> {
        return_errno!(ENOTDIR, "self is not dir");
    }

    fn poll(&self, mask: Events, _poller: Option<&Poller>) -> Events {
        let events = Events::IN | Events::OUT;
        mask & events
    }

    /// Get the file system of the inode
    fn fs(&self) -> Arc<dyn AsyncFileSystem>;

    /// Get the extension of this inode
    fn ext(&self) -> Option<&Extension> {
        None
    }

    /// This is used to implement dynamics cast.
    /// Simply return self in the implement of the function.
    fn as_any_ref(&self) -> &dyn Any;

    /// Lookup path from current inode.
    ///
    /// Do not follow symbolic links.
    async fn lookup(&self, path: &str) -> Result<Arc<dyn AsyncInode>> {
        let inode = self.lookup_follow(path, None).await?;
        Ok(inode)
    }

    /// Lookup path from current inode.
    ///
    /// The current inode must be a directory.
    ///
    /// The length of `path` cannot exceed PATH_MAX.
    /// If `path` ends with `/`, then the returned inode must be a directory inode.
    ///
    /// While looking up the inode, symbolic links will be followed for
    /// at most `max_follows` times, if it is given,
    async fn lookup_follow(
        &self,
        path: &str,
        max_follows: Option<usize>,
    ) -> Result<Arc<dyn AsyncInode>> {
        if self.metadata().await?.type_ != FileType::Dir {
            return_errno!(ENOTDIR, "not dir");
        }
        if path.len() > PATH_MAX {
            return_errno!(ENAMETOOLONG, "path name too long");
        }

        // To handle symlinks
        let mut link_path = String::new();
        let mut follows = 0;

        // Initialize the first inode and the relative path
        let (mut inode, mut relative_path) = if path.starts_with("/") {
            (self.fs().root_inode().await, path.trim_start_matches('/'))
        } else {
            (self.find(".").await?, path)
        };

        while !relative_path.is_empty() {
            let (next_name, path_remain, must_be_dir) =
                if let Some((prefix, suffix)) = relative_path.split_once('/') {
                    let suffix = suffix.trim_start_matches('/');
                    (prefix, suffix, true)
                } else {
                    (relative_path, "", false)
                };

            // Iterate next inode
            let next_inode = inode.find(next_name).await?;
            let next_inode_type = next_inode.metadata().await?.type_;

            // If next inode is a symlink, follow symlinks at most `max_follows` times.
            if max_follows.is_some() && next_inode_type == FileType::SymLink {
                if follows >= max_follows.unwrap() {
                    return_errno!(ELOOP, "too many symlinks");
                }
                let link_path_remain = {
                    let mut tmp_link_path = next_inode.read_link().await?;
                    if tmp_link_path.is_empty() {
                        return_errno!(ENOENT, "empty symlink path");
                    }
                    if !path_remain.is_empty() {
                        tmp_link_path += "/";
                        tmp_link_path += path_remain;
                    } else if must_be_dir {
                        tmp_link_path += "/";
                    }
                    tmp_link_path
                };

                // change the inode and relative path according to symlink
                if link_path_remain.starts_with("/") {
                    inode = inode.fs().root_inode().await;
                }
                link_path.clear();
                link_path.push_str(&link_path_remain.trim_start_matches('/'));
                relative_path = &link_path;
                follows += 1;
            } else {
                // If path ends with `/`, the inode must be a directory
                if must_be_dir && next_inode_type != FileType::Dir {
                    return_errno!(ENOTDIR, "not dir");
                }
                inode = next_inode;
                relative_path = path_remain;
            }
        }

        Ok(inode)
    }
}

impl dyn AsyncInode {
    /// Downcast the inode to specific struct
    pub fn downcast_ref<T: AsyncInode>(&self) -> Option<&T> {
        self.as_any_ref().downcast_ref::<T>()
    }
}
