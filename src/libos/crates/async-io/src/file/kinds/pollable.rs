use std::fmt::Debug;
use std::ops::Deref;

use futures::future::{self, BoxFuture};
use futures::prelude::*;

use crate::file::{AccessMode, SeekFrom, StatusFlags};
use crate::poll::{Events, Poller};
use crate::prelude::*;

/// An abstract for file APIs.
///
/// An implementation for this trait should make sure all read and write APIs
/// are non-blocking.
pub trait PollableFile: Debug + Sync + Send {
    fn read(&self, _buf: &mut [u8]) -> Result<usize> {
        return_errno!(EBADF, "not support read");
    }

    fn readv(&self, bufs: &mut [&mut [u8]]) -> Result<usize> {
        for buf in bufs {
            if buf.len() > 0 {
                return self.read(buf);
            }
        }
        Ok(0)
    }

    fn read_at(&self, _offset: usize, buf: &mut [u8]) -> Result<usize> {
        return_errno!(ESPIPE, "not support seek or read");
    }

    fn write(&self, _buf: &[u8]) -> Result<usize> {
        return_errno!(EBADF, "not support write");
    }

    fn writev(&self, bufs: &[&[u8]]) -> Result<usize> {
        for buf in bufs {
            if buf.len() > 0 {
                return self.write(buf);
            }
        }
        Ok(0)
    }

    fn write_at(&self, _offset: usize, buf: &[u8]) -> Result<usize> {
        return_errno!(ESPIPE, "not support seek or write");
    }

    fn flush(&self) -> BoxFuture<'_, Result<()>> {
        future::ready(Ok(())).boxed()
    }

    fn seek(&self, _pos: SeekFrom) -> Result<usize> {
        return_errno!(ESPIPE, "not support seek");
    }

    fn poll_by(&self, mask: Events, poller: Option<&mut Poller>) -> Events;

    /*
        fn ioctl(&self, cmd: &mut IoctlCmd) -> Result<i32> {
            return_op_unsupported_error!("ioctl")
        }
    */

    fn access_mode(&self) -> Result<AccessMode> {
        return_errno!(ENOSYS, "not support getting access mode");
    }

    fn set_access_mode(&self, new_mode: AccessMode) -> Result<()> {
        return_errno!(ENOSYS, "not support setting access mode");
    }

    fn status_flags(&self) -> Result<StatusFlags> {
        return_errno!(ENOSYS, "not support getting status flags");
    }

    fn set_status_flags(&self, new_status: StatusFlags) -> Result<()> {
        return_errno!(ENOSYS, "not support setting status flags");
    }
}

/// A wrapper type that extends a `PollableFile` object with async APIs.
pub struct Async<T> {
    file: T,
}

impl<F: PollableFile + ?Sized, T: Deref<Target = F>> Async<T> {
    pub fn new(file: T) -> Self {
        Self { file }
    }

    pub async fn read(&self, buf: &mut [u8]) -> Result<usize> {
        let is_nonblocking = self.is_nonblocking();

        // Fast path
        let res = self.file.read(buf);
        if Self::should_io_return(&res, is_nonblocking) {
            return res;
        }

        // Slow path
        let mask = Events::IN;
        let mut poller = Poller::new();
        loop {
            let events = self.poll_by(mask, Some(&mut poller));
            if events.contains(Events::IN) {
                let res = self.file.read(buf);
                if Self::should_io_return(&res, is_nonblocking) {
                    return res;
                }
            }
            poller.wait().await;
        }
    }

    pub async fn readv(&self, bufs: &mut [&mut [u8]]) -> Result<usize> {
        let is_nonblocking = self.is_nonblocking();

        // Fast path
        let res = self.file.readv(bufs);
        if Self::should_io_return(&res, is_nonblocking) {
            return res;
        }

        // Slow path
        let mask = Events::IN;
        let mut poller = Poller::new();
        loop {
            let events = self.poll_by(mask, Some(&mut poller));
            if events.contains(Events::IN) {
                let res = self.file.readv(bufs);
                if Self::should_io_return(&res, is_nonblocking) {
                    return res;
                }
            }
            poller.wait().await;
        }
    }

    pub async fn read_at(&self, offset: usize, buf: &mut [u8]) -> Result<usize> {
        let is_nonblocking = self.is_nonblocking();

        // Fast path
        let res = self.file.read_at(offset, buf);
        if Self::should_io_return(&res, is_nonblocking) {
            return res;
        }

        // Slow path
        let mask = Events::IN;
        let mut poller = Poller::new();
        loop {
            let events = self.poll_by(mask, Some(&mut poller));
            if events.contains(Events::IN) {
                let res = self.file.read_at(offset, buf);
                if Self::should_io_return(&res, is_nonblocking) {
                    return res;
                }
            }
            poller.wait().await;
        }
    }

    pub async fn write(&self, buf: &[u8]) -> Result<usize> {
        let is_nonblocking = self.is_nonblocking();

        // Fast path
        let res = self.file.write(buf);
        if Self::should_io_return(&res, is_nonblocking) {
            return res;
        }

        // Slow path
        let mask = Events::OUT;
        let mut poller = Poller::new();
        loop {
            let events = self.poll_by(mask, Some(&mut poller));
            if events.contains(Events::OUT) {
                let res = self.file.write(buf);
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
        let res = self.file.writev(bufs);
        if Self::should_io_return(&res, is_nonblocking) {
            return res;
        }

        // Slow path
        let mask = Events::OUT;
        let mut poller = Poller::new();
        loop {
            let events = self.poll_by(mask, Some(&mut poller));
            if events.contains(Events::OUT) {
                let res = self.file.writev(bufs);
                if Self::should_io_return(&res, is_nonblocking) {
                    return res;
                }
            }
            poller.wait().await;
        }
    }

    pub async fn write_at(&self, offset: usize, buf: &[u8]) -> Result<usize> {
        let is_nonblocking = self.is_nonblocking();

        // Fast path
        let res = self.file.write_at(offset, buf);
        if Self::should_io_return(&res, is_nonblocking) {
            return res;
        }

        // Slow path
        let mask = Events::OUT;
        let mut poller = Poller::new();
        loop {
            let events = self.poll_by(mask, Some(&mut poller));
            if events.contains(Events::OUT) {
                let res = self.file.write_at(offset, buf);
                if Self::should_io_return(&res, is_nonblocking) {
                    return res;
                }
            }
            poller.wait().await;
        }
    }

    pub async fn flush(&self) -> Result<()> {
        self.file.flush().await
    }

    pub fn seek(&self, pos: SeekFrom) -> Result<usize> {
        self.file.seek(pos)
    }

    pub fn poll_by(&self, mask: Events, poller: Option<&mut Poller>) -> Events {
        self.file.poll_by(mask, poller)
    }

    pub fn access_mode(&self) -> Result<AccessMode> {
        self.file.access_mode()
    }

    pub fn set_access_mode(&self, new_mode: AccessMode) -> Result<()> {
        self.file.set_access_mode(new_mode)
    }

    pub fn status_flags(&self) -> Result<StatusFlags> {
        self.file.status_flags()
    }

    pub fn set_status_flags(&self, new_status: StatusFlags) -> Result<()> {
        self.file.set_status_flags(new_status)
    }

    pub fn file(&self) -> &T {
        &self.file
    }

    pub fn unwrap(self) -> T {
        self.file
    }

    fn is_nonblocking(&self) -> bool {
        if let Ok(flags) = self.status_flags() {
            flags.contains(StatusFlags::O_NONBLOCK)
        } else {
            false
        }
    }

    fn should_io_return(res: &Result<usize>, is_nonblocking: bool) -> bool {
        is_nonblocking || Self::is_ok_or_not_egain(res)
    }

    fn is_ok_or_not_egain(res: &Result<usize>) -> bool {
        match res {
            Ok(_) => true,
            Err(e) => e.errno() != EAGAIN,
        }
    }
}

impl<T: std::fmt::Debug> std::fmt::Debug for Async<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Async").field("file", &self.file).finish()
    }
}

impl<F: PollableFile + ?Sized, T: Deref<Target = F> + Clone> Clone for Async<T> {
    fn clone(&self) -> Self {
        Self::new(self.file.clone())
    }
}

#[cfg(test)]
mod tests {
    use std::any::Any;
    use std::fmt::Debug;
    use std::sync::Arc;

    use super::*;
    use dummy_files::*;

    #[test]
    fn with_arc_dyn() {
        let foo = Arc::new(FooFile::new()) as Arc<dyn PollableFile>;
        let bar = Arc::new(BarFile::new()) as Arc<dyn PollableFile>;
        let async_foo = Async::new(foo);
        let async_bar = Async::new(bar);
        println!("foo file = {:?}", &async_foo);
        println!("bar file = {:?}", &async_bar);
    }

    mod dummy_files {
        use super::*;
        use crate::poll::Pollee;

        #[derive(Debug)]
        pub struct FooFile {
            pollee: Pollee,
        }

        impl FooFile {
            pub fn new() -> Self {
                Self {
                    pollee: Pollee::new(Events::empty()),
                }
            }
        }

        impl PollableFile for FooFile {
            fn poll_by(&self, mask: Events, poller: Option<&mut Poller>) -> Events {
                self.pollee.poll_by(mask, poller)
            }
        }

        #[derive(Debug)]
        pub struct BarFile {
            pollee: Pollee,
        }

        impl BarFile {
            pub fn new() -> Self {
                Self {
                    pollee: Pollee::new(Events::empty()),
                }
            }
        }

        impl PollableFile for BarFile {
            fn poll_by(&self, mask: Events, poller: Option<&mut Poller>) -> Events {
                self.pollee.poll_by(mask, poller)
            }
        }
    }
}
