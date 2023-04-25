use super::*;

pub use self::dir::Dir;
pub use self::file::File;
pub use self::symlink::SymLink;

mod dir;
mod file;
mod symlink;

#[async_trait]
pub trait ProcINode {
    async fn generate_data_in_bytes(&self) -> Result<Vec<u8>>;
}

#[async_trait]
pub trait DirProcINode {
    async fn find(&self, name: &str) -> Result<Arc<dyn AsyncInode>>;
    async fn iterate_entries(&self, ctx: &mut DirentWriterContext) -> Result<usize>;
}

#[macro_export]
macro_rules! write_first_two_entries {
    ($idx: expr, $ctx:expr, $file:expr) => {
        let idx = $idx;
        let file = $file;

        if idx == 0 {
            let this_inode = file.this.upgrade().unwrap();
            write_inode_entry!($ctx, ".", &this_inode);
        }
        if idx <= 1 {
            write_inode_entry!($ctx, "..", &file.parent);
        }
    };
}

#[macro_export]
macro_rules! write_inode_entry {
    ($ctx:expr, $name:expr, $inode:expr) => {
        let ctx = $ctx;
        let name = $name;
        let ino = $inode.metadata().await?.inode;
        let type_ = $inode.metadata().await?.type_;

        write_entry!(ctx, name, ino, type_);
    };
}

#[macro_export]
macro_rules! write_entry {
    ($ctx:expr, $name:expr, $ino:expr, $type_:expr) => {
        let ctx = $ctx;
        let name = $name;
        let ino = $ino;
        let type_ = $type_;

        if let Err(e) = ctx.write_entry(name, ino as u64, type_) {
            if ctx.written_len() == 0 {
                return_errno!(EINVAL, "write entry fail");
            } else {
                return Ok(ctx.written_len());
            }
        }
    };
}
