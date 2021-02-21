use std::any::Any;
use std::fmt::Debug;

use futures::future::{self, BoxFuture};
use futures::prelude::*;

use crate::poll::{Events, Poller};
use crate::prelude::*;

/// An abstract for file APIs.
///
/// An implementation for this trait should make sure all read and write APIs
/// are non-blocking.
pub trait File: Debug {
    fn read(&self, _buf: &mut [u8]) -> Result<usize> {
        return_errno!(EBADF, "the file cannot read");
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
        self.read(buf)
    }

    fn write(&self, _buf: &[u8]) -> Result<usize> {
        return_errno!(EBADF, "the file cannot write");
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
        self.write(buf)
    }

    fn flush(&self) -> BoxFuture<'_, Result<()>> {
        future::ready(Ok(())).boxed()
    }

    fn seek(&self, _pos: SeekFrom) -> Result<usize> {
        return_errno!(ESPIPE, "the file cannot seek");
    }

    fn poll_by(&self, mask: Events, poller: Option<&mut Poller>) -> Events;
    // TODO: add more APIs
    // * ioctl

    fn as_any(&self) -> &dyn Any;
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SeekFrom {
    Start(usize),
    End(usize),
    Current(isize),
}
