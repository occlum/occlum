use async_io::file::{Async, File};
use inherit_methods_macro::inherit_methods;

use std::sync::Weak;
use std::time::Duration;

use super::*;
use crate::fs::DiskFile;
use crate::net::SocketFile;
use crate::poll::EpollFile;
use crate::time::TimerFile;

// TODO: add fd to FileHandle?

/// A handle to a file-like object; similar to `Arc`, but for files.
///
/// # Design notes
///
/// Conceptually, `FileHandle` works like `Arc<dyn FileLike>` if we could have a trait named
/// `FileLike: Any` that abstracts the common characteristics of any file type. But we choose
/// not to do so. The primary reason is that `FileHandle` needs to have async methods, but
/// Rust does not support async methods in trait unless you are ok with incurring
/// an overhead of one heap allocation per call (I am not).
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
    Socket(Arc<SocketFile>),
    Epoll(Arc<EpollFile>),
    Timer(Arc<TimerFile>),
    Disk(Arc<DiskFile>),
    AsyncFileHandle(Arc<AsyncFileHandle>),
}

// Apply a function all variants of AnyFile enum.
macro_rules! apply_fn_on_any_file {
    ($any_file:expr, |$file:ident| { $($fn_body:tt)* }) => {{
        let any_file: &AnyFile = $any_file;
        match any_file {
            AnyFile::File($file) => {
                $($fn_body)*
            }
            AnyFile::Socket($file) => {
                $($fn_body)*
            }
            AnyFile::Epoll($file) => {
                $($fn_body)*
            }
            AnyFile::Timer($file) => {
                $($fn_body)*
            }
            AnyFile::Disk($file) => {
                $($fn_body)*
            }
            AnyFile::AsyncFileHandle($file) => {
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

    /// Create a file handle for a socket file.
    pub fn new_socket(file: SocketFile) -> Self {
        let any_file = AnyFile::Socket(Arc::new(file));
        Self::new(any_file)
    }

    /// Create a file handle for an epoll file.
    pub fn new_epoll(file: Arc<EpollFile>) -> Self {
        let any_file = AnyFile::Epoll(file);
        Self::new(any_file)
    }

    /// Create a file handle for an timer fd file.
    pub fn new_timer(file: TimerFile) -> Self {
        let any_file = AnyFile::Timer(Arc::new(file));
        Self::new(any_file)
    }

    /// Create a file handle for a disk file.
    pub fn new_disk(file: Arc<DiskFile>) -> Self {
        let any_file = AnyFile::Disk(file);
        Self::new(any_file)
    }

    pub fn new_async_file_handle(file: AsyncFileHandle) -> Self {
        let any_file = AnyFile::AsyncFileHandle(Arc::new(file));
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

    /// Perform Ioctl
    pub async fn ioctl(&self, cmd: &mut dyn IoctlCmd) -> Result<()> {
        apply_fn_on_any_file!(&self.0.file, |file| { file.ioctl(cmd).await })
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
    pub fn poll(&self, mask: Events, poller: Option<&Poller>) -> Events {
        apply_fn_on_any_file!(&self.0.file, |file| { file.poll(mask, poller) })
    }

    /// Register an observer for the file.
    pub fn register_observer(&self, observer: Arc<dyn Observer>, mask: Events) -> Result<()> {
        apply_fn_on_any_file!(&self.0.file, |file| {
            file.register_observer(observer, mask)
        })
    }

    /// Unregister an observer for the file.
    pub fn unregister_observer(&self, observer: &Arc<dyn Observer>) -> Result<Arc<dyn Observer>> {
        apply_fn_on_any_file!(&self.0.file, |file| { file.unregister_observer(observer) })
    }

    /// Returns the underlying socket file if it is one.
    pub fn as_socket_file(&self) -> Option<&SocketFile> {
        match &self.0.file {
            AnyFile::Socket(socket_file) => Some(socket_file),
            _ => None,
        }
    }

    // Returns the underlying epoll file if it is one.
    pub fn as_epoll_file(&self) -> Option<&EpollFile> {
        match &self.0.file {
            AnyFile::Epoll(epoll_file) => Some(epoll_file),
            _ => None,
        }
    }

    pub fn as_async_file(&self) -> Option<&Async<dyn File>> {
        match &self.0.file {
            AnyFile::File(async_file) => Some(async_file),
            _ => None,
        }
    }

    pub fn as_timer_file(&self) -> Option<&TimerFile> {
        match &self.0.file {
            AnyFile::Timer(timer_file) => Some(timer_file),
            _ => None,
        }
    }

    pub fn as_disk_file(&self) -> Option<&DiskFile> {
        match &self.0.file {
            AnyFile::Disk(disk_file) => Some(disk_file),
            _ => None,
        }
    }

    pub fn as_async_file_handle(&self) -> Option<&AsyncFileHandle> {
        match &self.0.file {
            AnyFile::AsyncFileHandle(async_file_handle) => Some(async_file_handle),
            _ => None,
        }
    }

    // Perform some clean work for some kinds of files when they close. Don't hold current thread's file table lock
    // when calling this function.
    pub async fn clean_for_close(self) -> Result<()> {
        match self.0.file {
            // Make sure the writes of disk files persist.
            //
            // Currently, disk files are the only types of files
            // that may have internal caches for updates and
            // requires explicit flushes to ensure the persist of the
            // updates.
            //
            // TODO: add a general-purpose mechanism to do async drop.
            // If we can support async drop, then there is no need to
            // do explicit cleanup/shutdown/flush when closing fd.
            AnyFile::Disk(disk_file) => {
                let _ = disk_file.flush().await;
            }
            // Make sure the socket async request completes so that when removing from the file table,
            // the host socket is actually dropped and closed.
            AnyFile::Socket(socket_file) => {
                let ref_count = Arc::strong_count(&socket_file);
                if ref_count == 1 {
                    let _ = socket_file.close().await;
                }
            }
            // Make sure the async inode flushing data when being closed.
            AnyFile::AsyncFileHandle(async_file_handle) => {
                let inode = async_file_handle.dentry().inode();
                if inode.as_sync_inode().is_none() {
                    let _ = inode.sync_all().await;
                }
                async_file_handle.release_range_locks();
            }
            _ => (),
        };

        Ok(())
    }

    /// Downgrade the file handle to its weak counterpart.
    pub fn downgrade(&self) -> WeakFileHandle {
        let any_weak_file = match &self.0.file {
            AnyFile::File(file) => AnyWeakFile::File(Arc::downgrade(file)),
            AnyFile::Socket(file) => AnyWeakFile::Socket(Arc::downgrade(file)),
            AnyFile::Epoll(file) => AnyWeakFile::Epoll(Arc::downgrade(file)),
            AnyFile::Timer(file) => AnyWeakFile::Timer(Arc::downgrade(file)),
            AnyFile::Disk(file) => AnyWeakFile::Disk(Arc::downgrade(file)),
            AnyFile::AsyncFileHandle(file) => AnyWeakFile::AsyncFileHandle(Arc::downgrade(file)),
        };
        WeakFileHandle(any_weak_file)
    }
}

impl PartialEq for FileHandle {
    fn eq(&self, other: &Self) -> bool {
        let rhs = (&self.0.file, &other.0.file);
        if let (AnyFile::File(self_file), AnyFile::File(other_file)) = rhs {
            Arc::as_ptr(self_file) == Arc::as_ptr(other_file)
        } else if let (AnyFile::Socket(self_socket), AnyFile::Socket(other_socket)) = rhs {
            Arc::as_ptr(self_socket) == Arc::as_ptr(other_socket)
        } else if let (AnyFile::Timer(self_timer), AnyFile::Timer(other_timer)) = rhs {
            Arc::as_ptr(self_timer) == Arc::as_ptr(other_timer)
        } else if let (AnyFile::Disk(self_disk), AnyFile::Disk(other_disk)) = rhs {
            Arc::as_ptr(self_disk) == Arc::as_ptr(other_disk)
        } else if let (AnyFile::AsyncFileHandle(self_file), AnyFile::AsyncFileHandle(other_file)) =
            rhs
        {
            Arc::as_ptr(self_file) == Arc::as_ptr(other_file)
        } else {
            false
        }
    }
}

/// The weak version of `FileHandle`. Similar to `Weak`, but for files.
#[derive(Clone, Debug)]
pub struct WeakFileHandle(AnyWeakFile);

#[derive(Clone, Debug)]
enum AnyWeakFile {
    File(Weak<Async<dyn File>>),
    Socket(Weak<SocketFile>),
    Epoll(Weak<EpollFile>),
    Timer(Weak<TimerFile>),
    Disk(Weak<DiskFile>),
    AsyncFileHandle(Weak<AsyncFileHandle>),
}

impl WeakFileHandle {
    /// Upgrade the weak file handle to its strong counterpart.
    pub fn upgrade(&self) -> Option<FileHandle> {
        match &self.0 {
            AnyWeakFile::File(weak) => weak
                .upgrade()
                .map(|arc| FileHandle::new(AnyFile::File(arc))),
            AnyWeakFile::Socket(weak) => weak
                .upgrade()
                .map(|arc| FileHandle::new(AnyFile::Socket(arc))),
            AnyWeakFile::Epoll(weak) => weak
                .upgrade()
                .map(|arc| FileHandle::new(AnyFile::Epoll(arc))),
            AnyWeakFile::Timer(weak) => weak
                .upgrade()
                .map(|arc| FileHandle::new(AnyFile::Timer(arc))),
            AnyWeakFile::Disk(weak) => weak
                .upgrade()
                .map(|arc| FileHandle::new(AnyFile::Disk(arc))),
            AnyWeakFile::AsyncFileHandle(weak) => weak
                .upgrade()
                .map(|arc| FileHandle::new(AnyFile::AsyncFileHandle(arc))),
        }
    }
}

impl PartialEq for WeakFileHandle {
    fn eq(&self, other: &Self) -> bool {
        let rhs = (&self.0, &other.0);
        if let (AnyWeakFile::File(self_file), AnyWeakFile::File(other_file)) = rhs {
            self_file.ptr_eq(&other_file)
        } else if let (AnyWeakFile::Socket(self_socket), AnyWeakFile::Socket(other_socket)) = rhs {
            self_socket.ptr_eq(&other_socket)
        } else if let (AnyWeakFile::Timer(self_timer), AnyWeakFile::Timer(other_timer)) = rhs {
            self_timer.ptr_eq(&other_timer)
        } else if let (AnyWeakFile::Disk(self_disk), AnyWeakFile::Disk(other_disk)) = rhs {
            self_disk.ptr_eq(&other_disk)
        } else if let (
            AnyWeakFile::AsyncFileHandle(self_file),
            AnyWeakFile::AsyncFileHandle(other_file),
        ) = rhs
        {
            self_file.ptr_eq(&other_file)
        } else {
            false
        }
    }
}
