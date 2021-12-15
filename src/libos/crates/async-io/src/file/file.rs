use std::fmt::Debug;
use std::marker::Unsize;
use std::mem::transmute;
use std::ops::CoerceUnsized;
use std::ops::Deref;

use futures::future::{self, BoxFuture};
use futures::prelude::*;
use inherit_methods_macro::inherit_methods;

use crate::event::{Events, Observer, Pollee, Poller};
use crate::file::{AccessMode, StatusFlags};
use crate::fs::StatBuf;
use crate::ioctl::IoctlCmd;
use crate::prelude::*;

/// An abstract for file APIs.
///
/// An implementation for this trait should make sure all read and write APIs
/// are non-blocking.
pub trait File: Debug + Sync + Send {
    fn read(&self, _buf: &mut [u8]) -> Result<usize> {
        return_errno!(EINVAL, "not support read");
    }

    fn readv(&self, bufs: &mut [&mut [u8]]) -> Result<usize> {
        for buf in bufs {
            if buf.len() > 0 {
                return self.read(buf);
            }
        }
        Ok(0)
    }

    fn write(&self, _buf: &[u8]) -> Result<usize> {
        return_errno!(EINVAL, "not support write");
    }

    fn writev(&self, bufs: &[&[u8]]) -> Result<usize> {
        for buf in bufs {
            if buf.len() > 0 {
                return self.write(buf);
            }
        }
        Ok(0)
    }

    fn poll(&self, mask: Events, poller: Option<&mut Poller>) -> Events {
        Events::empty()
    }

    fn register_observer(&self, observer: Arc<dyn Observer>, mask: Events) -> Result<()> {
        return_errno!(EINVAL, "this file does not support observers");
    }

    fn unregister_observer(&self, observer: &Arc<dyn Observer>) -> Result<Arc<dyn Observer>> {
        return_errno!(EINVAL, "this file does not support observers");
    }

    fn ioctl(&self, _cmd: &mut dyn IoctlCmd) -> Result<()> {
        return_errno!(EINVAL, "this file does not support ioctl");
    }

    fn access_mode(&self) -> AccessMode {
        AccessMode::O_RDWR
    }

    fn status_flags(&self) -> StatusFlags {
        StatusFlags::empty()
    }

    fn set_status_flags(&self, new_status: StatusFlags) -> Result<()> {
        return_errno!(ENOSYS, "not support setting status flags");
    }

    fn stat(&self) -> StatBuf {
        Default::default()
    }
}

/// A wrapper type that makes a `T: File`'s I/O methods _async_.
#[repr(transparent)]
pub struct Async<F: ?Sized>(F);

impl<F> Async<F> {
    pub fn new(file: F) -> Self {
        Self(file)
    }

    #[inline]
    pub fn info_file(self) -> F {
        self.0
    }
}

impl<F: File + ?Sized> Async<F> {
    pub async fn read(&self, buf: &mut [u8]) -> Result<usize> {
        let is_nonblocking = self.is_nonblocking();

        // Fast path
        let res = self.0.read(buf);
        if Self::should_io_return(&res, is_nonblocking) {
            return res;
        }

        // Slow path
        let mask = Events::IN;
        let mut poller = Poller::new();
        loop {
            let events = self.poll(mask, Some(&mut poller));
            if events.contains(Events::IN) {
                let res = self.0.read(buf);
                if Self::should_io_return(&res, is_nonblocking) {
                    return res;
                }
            }
            poller.wait().await?;
        }
    }

    pub async fn readv(&self, bufs: &mut [&mut [u8]]) -> Result<usize> {
        let is_nonblocking = self.is_nonblocking();

        // Fast path
        let res = self.0.readv(bufs);
        if Self::should_io_return(&res, is_nonblocking) {
            return res;
        }

        // Slow path
        let mask = Events::IN;
        let mut poller = Poller::new();
        loop {
            let events = self.poll(mask, Some(&mut poller));
            if events.contains(Events::IN) {
                let res = self.0.readv(bufs);
                if Self::should_io_return(&res, is_nonblocking) {
                    return res;
                }
            }
            poller.wait().await?;
        }
    }

    pub async fn write(&self, buf: &[u8]) -> Result<usize> {
        let is_nonblocking = self.is_nonblocking();

        // Fast path
        let res = self.0.write(buf);
        if Self::should_io_return(&res, is_nonblocking) {
            return res;
        }

        // Slow path
        let mask = Events::OUT;
        let mut poller = Poller::new();
        loop {
            let events = self.poll(mask, Some(&mut poller));
            if events.contains(Events::OUT) {
                let res = self.0.write(buf);
                if Self::should_io_return(&res, is_nonblocking) {
                    return res;
                }
            }
            poller.wait().await;
        }
    }

    pub async fn writev(&self, bufs: &[&[u8]]) -> Result<usize> {
        let is_nonblocking = self.is_nonblocking();

        // Fast path
        let res = self.0.writev(bufs);
        if Self::should_io_return(&res, is_nonblocking) {
            return res;
        }

        // Slow path
        let mask = Events::OUT;
        let mut poller = Poller::new();
        loop {
            let events = self.poll(mask, Some(&mut poller));
            if events.contains(Events::OUT) {
                let res = self.0.writev(bufs);
                if Self::should_io_return(&res, is_nonblocking) {
                    return res;
                }
            }
            poller.wait().await;
        }
    }

    #[inline]
    pub fn file(&self) -> &F {
        &self.0
    }

    fn should_io_return(res: &Result<usize>, is_nonblocking: bool) -> bool {
        is_nonblocking || !res.has_errno(EAGAIN)
    }

    fn is_nonblocking(&self) -> bool {
        let flags = self.status_flags();
        flags.contains(StatusFlags::O_NONBLOCK)
    }
}

// Implement methods inherited from File
#[inherit_methods(from = "self.0")]
#[rustfmt::skip]
impl<F: File + ?Sized> Async<F> {
    pub fn poll(&self, mask: Events, poller: Option<&mut Poller>) -> Events;
    pub fn register_observer(&self, observer: Arc<dyn Observer>, mask: Events) -> Result<()>;
    pub fn unregister_observer(&self, observer: &Arc<dyn Observer>) -> Result<Arc<dyn Observer>>;
    pub fn status_flags(&self) -> StatusFlags;
    pub fn set_status_flags(&self, new_status: StatusFlags) -> Result<()>;
    pub fn access_mode(&self) -> AccessMode;
    pub fn ioctl(&self, cmd: &mut dyn IoctlCmd) -> Result<()>;
    pub fn stat(&self) -> StatBuf;
}

impl<F: ?Sized + std::fmt::Debug> std::fmt::Debug for Async<F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl<F: Clone> Clone for Async<F> {
    fn clone(&self) -> Self {
        Self::new(self.0.clone())
    }
}

// Enable converting Async<T> to Async<dyn S>, where type T implements trait S.
impl<T: CoerceUnsized<U> + ?Sized, U: ?Sized> CoerceUnsized<Async<U>> for Async<T> {}

/// Convert a file-like type `T` into an async version, e.g., `Box<F>` to `Box<Async<F>>`
/// or `Arc<F>` to `Arc<Async<F>>`, where `F: File`.
pub trait IntoAsync {
    type AsyncFile;

    fn into_async(self) -> Self::AsyncFile;
}

impl<F: File + ?Sized> IntoAsync for Box<F> {
    type AsyncFile = Box<Async<F>>;

    fn into_async(self) -> Box<Async<F>> {
        // Safety. Async has a type wrapper with transparant memory representation.
        unsafe { transmute(self) }
    }
}

impl<F: File + ?Sized> IntoAsync for Arc<F> {
    type AsyncFile = Arc<Async<F>>;

    fn into_async(self) -> Arc<Async<F>> {
        // Safety. Async has a type wrapper with transparant memory representation.
        unsafe { transmute(self) }
    }
}

#[cfg(test)]
mod tests {
    use std::any::Any;
    use std::fmt::Debug;
    use std::sync::Arc;

    use super::*;

    #[test]
    fn new_async() {
        // Case 1
        let async_file: Async<DummyFile> = Async::new(DummyFile);
        let _ = async_file.access_mode();
        println!("{:?}", async_file);

        // Case 2
        let async_file: Arc<Async<dyn File>> = Arc::new(Async::new(DummyFile)) as _;
        let _ = async_file.access_mode();
        println!("{:?}", async_file);
    }

    #[test]
    fn into_async() {
        // Case 1
        let not_async: Arc<DummyFile> = Arc::new(DummyFile);
        let async_file: Arc<Async<DummyFile>> = not_async.into_async();
        let _ = async_file.access_mode();
        println!("{:?}", async_file);

        // Case 2
        let not_async: Arc<dyn File> = Arc::new(DummyFile) as _;
        let async_file: Arc<Async<dyn File>> = not_async.into_async();
        let _ = async_file.access_mode();
        println!("{:?}", async_file);
    }

    #[derive(Debug)]
    pub struct DummyFile;
    impl File for DummyFile {}
}
