use async_io::file::{Async, File};
use inherit_methods_macro::inherit_methods;

use std::sync::Weak;

use super::*;
use crate::net::SocketFile;

// TODO: add fd to FileHandle?

/// A handle to a file-like object; similar to `Arc`, but for files.
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
    File(Arc<Async<dyn File>>),
    Inode(Arc<AsyncInode>),
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
            let arc_async_file = Arc::new(Async::new(file)) as Arc<Async<dyn File>>;
            AnyFile::File(arc_async_file)
        };
        Self::new(any_file)
    }

    /// Create a file handle for an inode file.
    pub fn new_inode(file: InodeFile) -> Self {
        let any_file = AnyFile::Inode(Arc::new(AsyncInode::new(file)));
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

    /// Downgrade the file handle to its weak counterpart.
    pub fn downgrade(&self) -> WeakFileHandle {
        let any_weak_file = match &self.0.file {
            AnyFile::File(file) => AnyWeakFile::File(Arc::downgrade(file)),
            AnyFile::Inode(file) => AnyWeakFile::Inode(Arc::downgrade(file)),
            AnyFile::Socket(file) => AnyWeakFile::Socket(Arc::downgrade(file)),
        };
        WeakFileHandle(any_weak_file)
    }
}

impl PartialEq for FileHandle {
    fn eq(&self, other: &Self) -> bool {
        let rhs = (&self.0.file, &other.0.file);
        if let (AnyFile::File(self_file), AnyFile::File(other_file)) = rhs {
            Arc::as_ptr(self_file) == Arc::as_ptr(other_file)
        } else if let (AnyFile::Inode(self_inode), AnyFile::Inode(other_inode)) = rhs {
            Arc::as_ptr(self_inode) == Arc::as_ptr(other_inode)
        } else if let (AnyFile::Socket(self_socket), AnyFile::Socket(other_socket)) = rhs {
            Arc::as_ptr(self_socket) == Arc::as_ptr(other_socket)
        } else {
            false
        }
    }
}

/// A wrapper that makes `InodeFile`'s methods _async_.
#[derive(Debug)]
struct AsyncInode(InodeFile);

#[inherit_methods(from = "self.0")]
#[rustfmt::skip]
impl AsyncInode {
    pub fn new(inode: InodeFile) -> Self {
        Self(inode)
    }

    pub fn inner(&self) -> &InodeFile {
        &self.0
    }

    // Inherit methods from the inner InodeFile. Note that all I/O methods are
    // async wrappers of the original sync ones.
    pub async fn read(&self, buf: &mut [u8]) -> Result<usize>;
    pub async fn readv(&self, bufs: &mut [&mut [u8]]) -> Result<usize>;
    pub async fn write(&self, buf: &[u8]) -> Result<usize>;
    pub async fn writev(&self, bufs: &[&[u8]]) -> Result<usize>;
    pub fn poll_by(&self, mask: Events, poller: Option<&mut Poller>) -> Events;
    pub fn access_mode(&self) -> AccessMode;
    pub fn status_flags(&self) -> StatusFlags;
    pub fn set_status_flags(&self, new_status: StatusFlags) -> Result<()>;
}

/// The weak version of `FileHandle`. Similar to `Weak`, but for files.
#[derive(Clone, Debug)]
pub struct WeakFileHandle(AnyWeakFile);

#[derive(Clone, Debug)]
enum AnyWeakFile {
    File(Weak<Async<dyn File>>),
    Inode(Weak<AsyncInode>),
    Socket(Weak<SocketFile>),
}

impl WeakFileHandle {
    /// Upgrade the weak file handle to its strong counterpart.
    pub fn upgrade(&self) -> Option<FileHandle> {
        match &self.0 {
            AnyWeakFile::File(weak) => weak
                .upgrade()
                .map(|arc| FileHandle::new(AnyFile::File(arc))),
            AnyWeakFile::Inode(weak) => weak
                .upgrade()
                .map(|arc| FileHandle::new(AnyFile::Inode(arc))),
            AnyWeakFile::Socket(weak) => weak
                .upgrade()
                .map(|arc| FileHandle::new(AnyFile::Socket(arc))),
        }
    }
}
