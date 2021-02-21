use std::ops::Deref;

use super::{File, SeekFrom};
use crate::poll::{Events, Poller};
use crate::prelude::*;

/// A wrapper type that extends a `File` object with async APIs.
pub struct Async<T> {
    file: T,
}

impl<F: File + ?Sized, T: Deref<Target = F>> Async<T> {
    pub fn new(file: T) -> Self {
        Self { file }
    }

    pub async fn read(&self, buf: &mut [u8]) -> Result<usize> {
        // Fast path
        let res = self.file.read(buf);
        if is_ok_or_not_egain(&res) {
            return res;
        }

        // Slow path
        let mask = Events::IN;
        let mut poller = Poller::new();
        loop {
            let events = self.poll_by(mask, Some(&mut poller));
            if events.contains(Events::IN) {
                let res = self.file.read(buf);
                if is_ok_or_not_egain(&res) {
                    return res;
                }
            }
            poller.wait().await;
        }
    }

    pub async fn read_exact(&self, buf: &mut [u8]) -> Result<()> {
        let mut count = 0;
        while count < buf.len() {
            // TODO: handle EINTR
            let nbytes = self.read(&mut buf[count..]).await?;
            if nbytes == 0 {
                return_errno!(EINVAL, "unexpected EOF");
            }
            count += nbytes;
        }
        Ok(())
    }

    pub async fn read_at(&self, offset: usize, buf: &mut [u8]) -> Result<usize> {
        // Fast path
        let res = self.file.read_at(offset, buf);
        if is_ok_or_not_egain(&res) {
            return res;
        }

        // Slow path
        let mask = Events::IN;
        let mut poller = Poller::new();
        loop {
            let events = self.poll_by(mask, Some(&mut poller));
            if events.contains(Events::IN) {
                let res = self.file.read_at(offset, buf);
                if is_ok_or_not_egain(&res) {
                    return res;
                }
            }
            poller.wait().await;
        }
    }

    pub async fn read_exact_at(&self, offset: usize, buf: &mut [u8]) -> Result<()> {
        let mut count = 0;
        while count < buf.len() {
            // TODO: handle EINTR
            let nbytes = self.read_at(offset + count, &mut buf[count..]).await?;
            if nbytes == 0 {
                return_errno!(EINVAL, "unexpected EOF");
            }
            count += nbytes;
        }
        Ok(())
    }

    pub async fn write(&self, buf: &[u8]) -> Result<usize> {
        // Fast path
        let res = self.file.write(buf);
        if is_ok_or_not_egain(&res) {
            return res;
        }

        // Slow path
        let mask = Events::OUT;
        let mut poller = Poller::new();
        loop {
            let events = self.poll_by(mask, Some(&mut poller));
            if events.contains(Events::OUT) {
                let res = self.file.write(buf);
                if is_ok_or_not_egain(&res) {
                    return res;
                }
            }
            poller.wait().await;
        }
    }

    pub async fn write_exact(&self, buf: &[u8]) -> Result<()> {
        let mut count = 0;
        while count < buf.len() {
            // TODO: Handle EINTR
            count += self.write(&buf[count..]).await?;
        }
        Ok(())
    }

    pub async fn write_at(&self, offset: usize, buf: &[u8]) -> Result<usize> {
        // Fast path
        let res = self.file.write_at(offset, buf);
        if is_ok_or_not_egain(&res) {
            return res;
        }

        // Slow path
        let mask = Events::OUT;
        let mut poller = Poller::new();
        loop {
            let events = self.poll_by(mask, Some(&mut poller));
            if events.contains(Events::OUT) {
                let res = self.file.write_at(offset, buf);
                if is_ok_or_not_egain(&res) {
                    return res;
                }
            }
            poller.wait().await;
        }
    }

    pub async fn write_exact_at(&self, offset: usize, buf: &[u8]) -> Result<()> {
        let mut count = 0;
        while count < buf.len() {
            // TODO: Handle EINTR
            count += self.write_at(offset + count, &buf[count..]).await?;
        }
        Ok(())
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

    // TODO: add more APIs
    // * readv, read_at
    // * writev, write_at

    pub fn file(&self) -> &T {
        &self.file
    }
}

fn is_ok_or_not_egain<T>(res: &Result<T>) -> bool {
    match res {
        Ok(_) => true,
        Err(e) => e.errno() != EAGAIN,
    }
}

impl<T: std::fmt::Debug> std::fmt::Debug for Async<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Async").field("file", &self.file).finish()
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
        let foo = Arc::new(FooFile::new()) as Arc<dyn File>;
        let bar = Arc::new(BarFile::new()) as Arc<dyn File>;
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

        impl File for FooFile {
            fn poll_by(&self, mask: Events, poller: Option<&mut Poller>) -> Events {
                self.pollee.poll_by(mask, poller)
            }

            fn as_any(&self) -> &dyn Any {
                self as &dyn Any
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

        impl File for BarFile {
            fn poll_by(&self, mask: Events, poller: Option<&mut Poller>) -> Events {
                self.pollee.poll_by(mask, poller)
            }

            fn as_any(&self) -> &dyn Any {
                self as &dyn Any
            }
        }
    }
}
