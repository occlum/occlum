use async_io::file::{Async as AsyncFile, File};

use super::*;
use crate::net::SocketFile;

// TODO: fix the unncessary double-arc
// TODO: add fd to FileHandle?

#[derive(Clone, Debug)]
pub struct FileHandle(Arc<Inner>);

#[derive(Debug)]
struct Inner {
    file: AnyFile,
}

#[derive(Clone, Debug)]
enum AnyFile {
    File(AsyncFile<Arc<dyn File>>),
    Inode(AsyncInode),
    Socket(Arc<SocketFile>),
}

// Apply a function all variants of AnyFile enum.
macro_rules! apply_fn_on_any_file {
    ($any_file:expr, |$file:ident| { $($fn_body:tt)* }) => {{
        let any_file: &AnyFile = $any_file;
        match any_file {
            AnyFile::File($file) => {
                $($fn_body)*
            }
            AnyFile::Inode($file) => {
                $($fn_body)*
            }
            AnyFile::Socket($file) => {
                $($fn_body)*
            }
        }
    }}
}

impl FileHandle {
    pub fn new_file<F: File + 'static>(file: F) -> Self {
        let any_file = {
            let new_arc = Arc::new(file) as Arc<dyn File>;
            let new_async = AsyncFile::new(new_arc);
            AnyFile::File(new_async)
        };
        Self::new(any_file)
    }

    pub fn new_inode(file: InodeFile) -> Self {
        let any_file = AnyFile::Inode(AsyncInode::new(file));
        Self::new(any_file)
    }

    pub fn new_socket(file: SocketFile) -> Self {
        let any_file = AnyFile::Socket(Arc::new(file));
        Self::new(any_file)
    }

    fn new(file: AnyFile) -> Self {
        let inner = Inner { file };
        Self(Arc::new(inner))
    }

    pub async fn read(&self, buf: &mut [u8]) -> Result<usize> {
        apply_fn_on_any_file!(&self.0.file, |file| { file.read(buf).await })
    }

    pub async fn readv(&self, bufs: &mut [&mut [u8]]) -> Result<usize> {
        apply_fn_on_any_file!(&self.0.file, |file| { file.readv(bufs).await })
    }

    pub async fn write(&self, buf: &[u8]) -> Result<usize> {
        apply_fn_on_any_file!(&self.0.file, |file| { file.write(buf).await })
    }

    pub async fn writev(&self, bufs: &[&[u8]]) -> Result<usize> {
        apply_fn_on_any_file!(&self.0.file, |file| { file.writev(bufs).await })
    }

    pub fn access_mode(&self) -> AccessMode {
        apply_fn_on_any_file!(&self.0.file, |file| { file.access_mode() })
    }

    pub fn status_flags(&self) -> StatusFlags {
        apply_fn_on_any_file!(&self.0.file, |file| { file.status_flags() })
    }

    pub fn set_status_flags(&self, new_flags: StatusFlags) -> Result<()> {
        apply_fn_on_any_file!(&self.0.file, |file| { file.set_status_flags(new_flags) })
    }

    pub fn poll_by(&self, mask: Events, poller: Option<&mut Poller>) -> Events {
        apply_fn_on_any_file!(&self.0.file, |file| { file.poll_by(mask, poller) })
    }

    pub fn as_inode_file(&self) -> Option<&InodeFile> {
        match &self.0.file {
            AnyFile::Inode(inode_file) => Some(inode_file.inner()),
            _ => None,
        }
    }

    pub fn as_socket_file(&self) -> Option<&SocketFile> {
        match &self.0.file {
            AnyFile::Socket(socket_file) => Some(socket_file),
            _ => None,
        }
    }
}

impl PartialEq for FileHandle {
    fn eq(&self, other: &Self) -> bool {
        Arc::as_ptr(&self.0) == Arc::as_ptr(&other.0)
    }
}
