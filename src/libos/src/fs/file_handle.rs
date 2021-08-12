use async_io::file::{Async as AsyncFile, File};

use super::*;
use crate::net::SocketFile;

// TODO: add fd to FileHandle?

/// A handle to a file-like object.
///
/// # Design notes
///
/// Conceptually, `FileHandle` works like `Arc<dyn FileLike>` if we could have a trait named
/// `FileLike: Any` that abstracts the common characteristics of any file type. But we choose
/// not to do so. The primary reason is that `FileHandle` needs to have async methods, but
/// Rust does not support async methods in trait unless you are ok with incurring
/// an overhead of one heap allocationn per call (I am not).
/// For more info, check out [the async-trait crate](https://crates.io/crates/async-trait).
///
/// Internally, `FileHandle` is implemented with an enum. Using enums is sufficient to achieve
/// the polyphoysim we want here since the LibOS can foresee all possible types of files that
/// can be represented by `FileHandle`.
#[derive(Clone, Debug)]
pub struct FileHandle(Inner);

#[derive(Clone, Debug)]
struct Inner {
    file: AnyFile,
    // More fields are expected in the future
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
    /// Create a file handle for an object of `F: File`.
    pub fn new_file<F: File + 'static>(file: F) -> Self {
        let any_file = {
            let new_arc = Arc::new(file) as Arc<dyn File>;
            let new_async = AsyncFile::new(new_arc);
            AnyFile::File(new_async)
        };
        Self::new(any_file)
    }

    /// Create a file handle for an inode file.
    pub fn new_inode(file: InodeFile) -> Self {
        let any_file = AnyFile::Inode(AsyncInode::new(file));
        Self::new(any_file)
    }

    /// Create a file handle for a socket file.
    pub fn new_socket(file: SocketFile) -> Self {
        let any_file = AnyFile::Socket(Arc::new(file));
        Self::new(any_file)
    }

    fn new(file: AnyFile) -> Self {
        let inner = Inner { file };
        Self(inner)
    }

    /// Read some data into a buffer.
    pub async fn read(&self, buf: &mut [u8]) -> Result<usize> {
        apply_fn_on_any_file!(&self.0.file, |file| { file.read(buf).await })
    }

    /// Read some data into a set of buffers.
    pub async fn readv(&self, bufs: &mut [&mut [u8]]) -> Result<usize> {
        apply_fn_on_any_file!(&self.0.file, |file| { file.readv(bufs).await })
    }

    /// Write the data from a buffer.
    pub async fn write(&self, buf: &[u8]) -> Result<usize> {
        apply_fn_on_any_file!(&self.0.file, |file| { file.write(buf).await })
    }

    /// Write the data from a set of buffers.
    pub async fn writev(&self, bufs: &[&[u8]]) -> Result<usize> {
        apply_fn_on_any_file!(&self.0.file, |file| { file.writev(bufs).await })
    }

    /// Returns the access mode of the file.
    pub fn access_mode(&self) -> AccessMode {
        apply_fn_on_any_file!(&self.0.file, |file| { file.access_mode() })
    }

    /// Returns the status flags of the file.
    pub fn status_flags(&self) -> StatusFlags {
        apply_fn_on_any_file!(&self.0.file, |file| { file.status_flags() })
    }

    /// Set the status flags of the file.
    pub fn set_status_flags(&self, new_flags: StatusFlags) -> Result<()> {
        apply_fn_on_any_file!(&self.0.file, |file| { file.set_status_flags(new_flags) })
    }

    /// Poll the I/O readiness of the file.
    pub fn poll(&self, mask: Events, poller: Option<&mut Poller>) -> Events {
        apply_fn_on_any_file!(&self.0.file, |file| { file.poll_by(mask, poller) })
    }

    /// Returns the underlying inode file if there is one.
    pub fn as_inode_file(&self) -> Option<&InodeFile> {
        match &self.0.file {
            AnyFile::Inode(inode_file) => Some(inode_file.inner()),
            _ => None,
        }
    }

    /// Returns the underlying socket file if there is one.
    pub fn as_socket_file(&self) -> Option<&SocketFile> {
        match &self.0.file {
            AnyFile::Socket(socket_file) => Some(socket_file),
            _ => None,
        }
    }
}

impl PartialEq for FileHandle {
    fn eq(&self, other: &Self) -> bool {
        let rhs = (&self.0.file, &other.0.file);
        if let (AnyFile::File(self_file), AnyFile::File(other_file)) = rhs {
            Arc::as_ptr(self_file.inner()) == Arc::as_ptr(other_file.inner())
        } else if let (AnyFile::Inode(self_inode), AnyFile::Inode(other_inode)) = rhs {
            Arc::as_ptr(self_inode.inner()) == Arc::as_ptr(other_inode.inner())
        } else if let (AnyFile::Socket(self_socket), AnyFile::Socket(other_socket)) = rhs {
            Arc::as_ptr(self_socket) == Arc::as_ptr(other_socket)
        } else {
            false
        }
    }
}
