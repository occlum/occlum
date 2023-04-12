use crate::inode::AsyncInode;
use crate::prelude::*;

use async_io::fs::{FsInfo, FsMac};
use async_trait::async_trait;

/// Abstract Async FileSystem
#[async_trait]
pub trait AsyncFileSystem: Sync + Send {
    /// Sync all data to the storage
    async fn sync(&self) -> Result<()>;

    /// Get the root INode of the file system
    async fn root_inode(&self) -> Arc<dyn AsyncInode>;

    /// Get the file system information
    async fn info(&self) -> FsInfo;

    /// Get the MAC of the file system
    async fn mac(&self) -> FsMac {
        Default::default()
    }
}
