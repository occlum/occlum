use super::*;

pub use self::dir::Dir;
pub use self::file::File;
pub use self::symlink::SymLink;

mod dir;
mod file;
mod symlink;

pub trait ProcINode {
    fn generate_data_in_bytes(&self) -> vfs::Result<Vec<u8>>;
}

pub trait DirProcINode {
    fn find(&self, name: &str) -> vfs::Result<Arc<dyn INode>>;
    fn get_entry(&self, id: usize) -> vfs::Result<String>;
    fn iterate_entries(&self, offset: usize, visitor: &mut dyn DirentVisitor)
        -> vfs::Result<usize>;
}

#[macro_export]
macro_rules! impl_inode_for_file_or_symlink {
    () => {
        fn read_at(&self, offset: usize, buf: &mut [u8]) -> vfs::Result<usize> {
            let data = self.inner().generate_data_in_bytes()?;
            let start = data.len().min(offset);
            let end = data.len().min(offset + buf.len());
            let len = end - start;
            buf[0..len].copy_from_slice(&data[start..end]);
            Ok(len)
        }

        fn write_at(&self, offset: usize, buf: &[u8]) -> vfs::Result<usize> {
            Err(vfs::FsError::PermError)
        }

        fn set_metadata(&self, metadata: &Metadata) -> vfs::Result<()> {
            Err(vfs::FsError::PermError)
        }

        fn sync_all(&self) -> vfs::Result<()> {
            Ok(())
        }

        fn sync_data(&self) -> vfs::Result<()> {
            Ok(())
        }

        fn find(&self, name: &str) -> vfs::Result<Arc<dyn INode>> {
            Err(FsError::NotDir)
        }

        fn get_entry(&self, id: usize) -> vfs::Result<String> {
            Err(FsError::NotDir)
        }

        fn iterate_entries(
            &self,
            offset: usize,
            visitor: &mut dyn DirentVisitor,
        ) -> vfs::Result<usize> {
            Err(FsError::NotDir)
        }

        fn as_any_ref(&self) -> &dyn Any {
            self
        }
    };
}

#[macro_export]
macro_rules! visit_first_two_entries {
    ($visitor:expr, $file:expr, $offset: expr) => {
        use rcore_fs::visit_inode_entry;
        let file = $file;

        let offset = **$offset;
        if offset == 0 {
            let this_inode = file.this.upgrade().unwrap();
            visit_inode_entry!($visitor, ".", &this_inode, $offset);
        }
        let offset = **$offset;
        if offset == 1 {
            visit_inode_entry!($visitor, "..", &file.parent, $offset);
        }
    };
}
