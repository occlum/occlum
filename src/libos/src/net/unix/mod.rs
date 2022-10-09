use super::socket_file::UntrustedUnixStream;
use super::*;
use crate::prelude::*;
use crate::util::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use self::trusted::Stream as TrustedStream;
use self::trusted::TrustedAddr;
use self::untrusted::UNTRUSTED_SOCKS;
use async_io::event::{Events, Observer, Poller};
use async_io::file::StatusFlags;
use async_io::ioctl::IoctlCmd;
use async_io::socket::Shutdown;
use async_io::socket::{RecvFlags, SendFlags};

pub mod trusted;
pub mod untrusted;

#[derive(Debug)]
pub struct UnixStream {
    inner: RwLock<StreamInner>,
}

#[derive(Debug)]
enum StreamInner {
    Trusted(TrustedStream),
    Untrusted(UntrustedUnixStream),
}

// Apply a function to all variants of AnySocket enum.
macro_rules! apply_fn_on_any_stream {
    ($any_stream:expr, |$inner:ident| { $($fn_body:tt)* }) => {{
        let any_stream: RwLockReadGuard<StreamInner> = $any_stream;
        match &*any_stream {
            StreamInner::Trusted($inner) => {
                $($fn_body)*
            }
            StreamInner::Untrusted($inner) => {
                $($fn_body)*
            }
        }
    }}
}

impl UnixStream {
    pub fn new_trusted(nonblocking: bool) -> Self {
        let trusted_stream = TrustedStream::new(nonblocking);
        let inner = StreamInner::Trusted(trusted_stream);
        Self {
            inner: RwLock::new(inner),
        }
    }

    fn inner(&self) -> RwLockReadGuard<StreamInner> {
        self.inner.read().unwrap()
    }

    fn inner_mut(&self) -> RwLockWriteGuard<StreamInner> {
        self.inner.write().unwrap()
    }

    fn is_nonblocking(&self) -> bool {
        match &*self.inner() {
            StreamInner::Trusted(stream) => stream.nonblocking(),
            StreamInner::Untrusted(u_stream) => {
                u_stream.status_flags().contains(StatusFlags::O_NONBLOCK)
            }
        }
    }

    pub fn domain(&self) -> Domain {
        apply_fn_on_any_stream!(self.inner(), |stream| { stream.domain() })
    }

    fn get_host_socket_file_path(
        libos_path: &TrustedAddr,
        host_path: UnixAddr,
        is_socket_file: bool,
    ) -> UnixAddr {
        if is_socket_file {
            host_path
        } else {
            // unix socket file name is not specified in Occlum.json. Use the same basename as the bind addr.
            let dir_path = host_path
                .get_path_name()
                .expect("This must be a path name string")
                .to_owned();
            let file_base_name = libos_path.as_str().unwrap().rsplit_once('/').unwrap().1;
            trace!("socket file file_base_name = {:?}", file_base_name);
            let socket_file_path = dir_path + "/" + file_base_name;
            trace!("socket file path = {:?}", socket_file_path);
            UnixAddr::new_with_path_name(&socket_file_path)
        }
    }

    pub async fn bind(&self, addr: &mut TrustedAddr) -> Result<()> {
        debug!("bind addr = {:?}", addr);
        // Distinguish if the real socket is internal trusted or cross-world untrusted
        if let Some((host_path, is_socket_file)) = addr.get_crossworld_sock_path() {
            let nonblocking = self.is_nonblocking();

            // Create untrusted socket end
            let untrusted_sock = UntrustedUnixStream::new(nonblocking)?;

            // Bind the Host FS address
            let host_addr = Self::get_host_socket_file_path(addr, host_path, is_socket_file);
            trace!(
                "bind cross world sock: libos path: {:?}, host path: {:?}",
                addr,
                host_addr
            );
            addr.bind_untrusted_addr(&host_addr).await?; // bind two address
            untrusted_sock.bind(&host_addr)?;

            // replace the trusted socket end with untrusted socket end
            let mut inner = self.inner_mut();

            *inner = StreamInner::Untrusted(untrusted_sock);

            return Ok(());
        }

        if let StreamInner::Trusted(stream) = &*self.inner() {
            // Create a file in libos FS
            addr.bind_addr().await?;
            return stream.bind(addr);
        }

        unreachable!();
    }

    pub async fn connect(&self, addr: &TrustedAddr) -> Result<()> {
        if let Some((host_path, is_socket_file)) = addr.get_crossworld_sock_path() {
            let nonblocking = self.is_nonblocking();

            // Create untrusted socket end
            let untrusted_sock = UntrustedUnixStream::new(nonblocking)?;

            let host_addr = Self::get_host_socket_file_path(addr, host_path, is_socket_file);
            trace!(
                "connect cross world sock: libos path: {:?}, host path: {:?}",
                addr,
                host_addr
            );
            untrusted_sock.connect(&host_addr).await?;

            // replace the trusted socket end with untrusted socket end
            let mut inner = self.inner_mut();

            *inner = StreamInner::Untrusted(untrusted_sock);
            return Ok(());
        }

        if let StreamInner::Trusted(stream) = &*self.inner() {
            // Init inode for libos local file
            let mut addr = addr.clone();
            addr.try_init_inode().await?;

            return stream.connect(&addr).await;
        }

        unreachable!();
    }

    pub fn listen(&self, backlog: u32) -> Result<()> {
        apply_fn_on_any_stream!(self.inner(), |stream| { stream.listen(backlog) })
    }

    pub async fn accept(&self, nonblocking: bool) -> Result<Self> {
        match &*self.inner() {
            StreamInner::Trusted(stream_t) => {
                let accepted_stream = stream_t.accept(nonblocking).await?;
                let inner = StreamInner::Trusted(accepted_stream);
                Ok(Self {
                    inner: RwLock::new(inner),
                })
            }
            StreamInner::Untrusted(stream_u) => {
                let accepted_stream = stream_u.accept(nonblocking).await?;
                let inner = StreamInner::Untrusted(accepted_stream);
                Ok(Self {
                    inner: RwLock::new(inner),
                })
            }
        }
    }

    pub async fn recvmsg(
        &self,
        buf: &mut [&mut [u8]],
        flags: RecvFlags,
        control: Option<&mut [u8]>,
    ) -> Result<(usize, Option<UnixAddr>)> {
        apply_fn_on_any_stream!(self.inner(), |stream| {
            stream.recvmsg(buf, flags, None).await
        })
    }

    pub async fn sendmsg(&self, bufs: &[&[u8]], flags: SendFlags) -> Result<usize> {
        apply_fn_on_any_stream!(self.inner(), |stream| { stream.sendmsg(bufs, flags).await })
    }

    pub fn peer_addr(&self) -> Result<AnyAddr> {
        match &*self.inner() {
            StreamInner::Trusted(stream_t) => Ok(AnyAddr::TrustedUnix(stream_t.peer_addr()?)),
            StreamInner::Untrusted(stream_u) => Ok(AnyAddr::Unix(stream_u.peer_addr()?)),
        }
    }

    pub fn addr(&self) -> Result<AnyAddr> {
        match &*self.inner() {
            StreamInner::Trusted(stream_t) => Ok(AnyAddr::TrustedUnix(stream_t.addr()?)),
            StreamInner::Untrusted(stream_u) => Ok(AnyAddr::Unix(stream_u.addr()?)),
        }
    }

    pub async fn shutdown(&self, how: Shutdown) -> Result<()> {
        apply_fn_on_any_stream!(self.inner(), |stream| { stream.shutdown(how).await })
    }
}

// Implement the common methods required by FileHandle
impl UnixStream {
    pub async fn read(&self, buf: &mut [u8]) -> Result<usize> {
        apply_fn_on_any_stream!(self.inner(), |stream| { stream.read(buf).await })
    }

    pub async fn readv(&self, bufs: &mut [&mut [u8]]) -> Result<usize> {
        apply_fn_on_any_stream!(self.inner(), |stream| { stream.readv(bufs).await })
    }

    pub async fn write(&self, buf: &[u8]) -> Result<usize> {
        apply_fn_on_any_stream!(self.inner(), |stream| { stream.write(buf).await })
    }

    pub async fn writev(&self, bufs: &[&[u8]]) -> Result<usize> {
        apply_fn_on_any_stream!(self.inner(), |stream| { stream.writev(bufs).await })
    }

    pub fn status_flags(&self) -> StatusFlags {
        apply_fn_on_any_stream!(self.inner(), |stream| { stream.status_flags() })
    }

    pub fn set_status_flags(&self, new_flags: StatusFlags) -> Result<()> {
        apply_fn_on_any_stream!(self.inner(), |stream| {
            stream.set_status_flags(new_flags)
        })
    }

    pub fn poll(&self, mask: Events, poller: Option<&Poller>) -> Events {
        apply_fn_on_any_stream!(self.inner(), |stream| { stream.poll(mask, poller) })
    }

    pub fn register_observer(&self, observer: Arc<dyn Observer>, mask: Events) -> Result<()> {
        apply_fn_on_any_stream!(self.inner(), |stream| {
            stream.register_observer(observer, mask)
        })
    }

    pub fn unregister_observer(&self, observer: &Arc<dyn Observer>) -> Result<Arc<dyn Observer>> {
        apply_fn_on_any_stream!(self.inner(), |stream| {
            stream.unregister_observer(observer)
        })
    }

    pub fn ioctl(&self, cmd: &mut dyn IoctlCmd) -> Result<()> {
        apply_fn_on_any_stream!(self.inner(), |stream| { stream.ioctl(cmd) })
    }
}
