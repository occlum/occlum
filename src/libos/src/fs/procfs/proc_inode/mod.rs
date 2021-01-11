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

        fn as_any_ref(&self) -> &dyn Any {
            self
        }
    };
}
