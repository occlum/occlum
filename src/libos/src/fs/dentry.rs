use super::*;

/// The Dentry is used to speed up the pathname lookup
pub struct Dentry {
    inode: Arc<dyn AsyncInode>,
    abs_path: String,
}

impl Dentry {
    pub fn new(inode: Arc<dyn AsyncInode>, abs_path: String) -> Self {
        Self { inode, abs_path }
    }

    pub fn inode(&self) -> &Arc<dyn AsyncInode> {
        &self.inode
    }

    // TODO: lookup parent dentry to get the absolute path
    pub fn abs_path(&self) -> &str {
        &self.abs_path
    }
}

impl Drop for Dentry {
    fn drop(&mut self) {
        let inode = self.inode.clone();
        async_rt::task::spawn(async move {
            inode.sync_all().await;
        });
    }
}
