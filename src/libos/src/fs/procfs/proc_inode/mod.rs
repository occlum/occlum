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
    fn iterate_entries(&self, ctx: &mut DirentWriterContext) -> vfs::Result<usize>;
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

        fn iterate_entries(&self, ctx: &mut DirentWriterContext) -> vfs::Result<usize> {
            Err(FsError::NotDir)
        }

        fn as_any_ref(&self) -> &dyn Any {
            self
        }
    };
}

#[macro_export]
macro_rules! write_first_two_entries {
    ($idx: expr, $ctx:expr, $file:expr, $total_written:expr) => {
        let idx = $idx;
        let file = $file;

        if idx == 0 {
            let this_inode = file.this.upgrade().unwrap();
            write_inode_entry!($ctx, ".", &this_inode, $total_written);
        }
        if idx <= 1 {
            write_inode_entry!($ctx, "..", &file.parent, $total_written);
        }
    };
}

#[macro_export]
macro_rules! write_inode_entry {
    ($ctx:expr, $name:expr, $inode:expr, $total_written:expr) => {
        let ctx = $ctx;
        let name = $name;
        let ino = $inode.metadata()?.inode;
        let type_ = $inode.metadata()?.type_;
        let total_written = $total_written;

        write_entry!(ctx, name, ino, type_, total_written);
    };
}

#[macro_export]
macro_rules! write_entry {
    ($ctx:expr, $name:expr, $ino:expr, $type_:expr, $total_written:expr) => {
        let ctx = $ctx;
        let name = $name;
        let ino = $ino;
        let type_ = $type_;
        let total_written = $total_written;

        match ctx.write_entry(name, ino as u64, type_) {
            Ok(written_len) => {
                *total_written += written_len;
            }
            Err(e) => {
                if *total_written == 0 {
                    return Err(e);
                } else {
                    return Ok(*total_written);
                }
            }
        }
    };
}
