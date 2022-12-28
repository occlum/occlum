use super::address_space::ADDRESS_SPACE;
use super::sock_end::{end_pair, SockEnd};
use super::*;
use async_io::event::{Events, Observer, Poller};
use async_io::file::StatusFlags;
use async_io::ioctl::IoctlCmd;
use async_io::socket::{MsgFlags, SetRecvTimeoutCmd, SetSendTimeoutCmd, Shutdown, Timeout};
use async_io::util::channel::Channel;
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Unix socket based on trusted channel. It has three statuses: unconnected, listening and connected.  When a
/// socket is created, it is in unconnected status.  It will transfer to listening after listen is
/// called and connected after connect is called. A socket in connected status can be obtained
/// through a listening socket calling accept. Listening and connected are ultimate statuses. They
/// will not transfer to other statuses.
pub struct Stream {
    inner: SgxMutex<Status>,
}

impl Stream {
    pub fn new(nonblocking: bool) -> Self {
        Self {
            inner: SgxMutex::new(Status::Idle(Info::new(nonblocking))),
        }
    }

    pub(super) fn inner(&self) -> SgxMutexGuard<'_, Status> {
        self.inner.lock().unwrap()
    }

    pub async fn read(&self, buf: &mut [u8]) -> Result<usize> {
        self.readv(&mut [buf]).await
    }

    pub async fn readv(&self, bufs: &mut [&mut [u8]]) -> Result<usize> {
        self.recvmsg(bufs, RecvFlags::empty())
            .await
            .map(|ret| ret.0)
    }

    /// Linux behavior:
    /// Unlike datagram socket, `recvfrom` / `recvmsg` of stream socket will
    /// ignore the address even if user specified it.
    /// TODO: handle flags
    pub async fn recvmsg(
        &self,
        bufs: &mut [&mut [u8]],
        flags: RecvFlags,
    ) -> Result<(usize, Option<UnixAddr>, MsgFlags)> {
        let addr = {
            let trusted_addr = self.peer_addr().ok();
            debug!("recvfrom {:?}", trusted_addr);
            if let Some(addr) = trusted_addr {
                match addr.inner() {
                    UnixAddr::Pathname(path) => Some(UnixAddr::Pathname(path.clone())),
                    UnixAddr::Abstract(name) => Some(UnixAddr::Abstract(name.clone())),
                    UnixAddr::Unnamed => None,
                }
            } else {
                None
            }
        };

        let connected_stream = {
            match &*self.inner() {
                Status::Connected(endpoint) => endpoint.clone(),
                _ => {
                    return_errno!(EINVAL, "the socket is not connected");
                }
            }
        };
        let data_len = connected_stream.readv(bufs).await?;

        Ok((data_len, addr, MsgFlags::empty()))
    }

    pub async fn write(&self, buf: &[u8]) -> Result<usize> {
        self.writev(&mut [buf]).await
    }

    pub async fn writev(&self, bufs: &[&[u8]]) -> Result<usize> {
        self.sendmsg(bufs, SendFlags::empty()).await
    }

    // TODO: handle flags
    pub async fn sendmsg(&self, bufs: &[&[u8]], flags: SendFlags) -> Result<usize> {
        let addr = self.peer_addr().ok();
        debug!("sendmsg to {:?}", addr);

        let connected_stream = {
            match &*self.inner() {
                Status::Connected(endpoint) => endpoint.clone(),
                _ => {
                    return_errno!(EINVAL, "the socket is not connected");
                }
            }
        };
        let data_len = connected_stream.writev(bufs).await?;

        Ok(data_len)
    }

    pub fn status_flags(&self) -> StatusFlags {
        // Only support O_NONBLOCK
        if self.nonblocking() {
            StatusFlags::O_NONBLOCK
        } else {
            StatusFlags::empty()
        }
    }

    pub fn set_status_flags(&self, new_flags: StatusFlags) -> Result<()> {
        // Only support O_NONBLOCK
        let nonblocking = new_flags.is_nonblocking();
        self.set_nonblocking(nonblocking);
        Ok(())
    }

    pub fn poll(&self, mask: Events, poller: Option<&Poller>) -> Events {
        match &*self.inner() {
            Status::Idle(info) => Events::OUT | Events::HUP,
            Status::Connected(endpoint) => endpoint.poll(mask, poller),
            Status::Listening(addr) => {
                if let Some(listener) = ADDRESS_SPACE.get_listener_ref(addr) {
                    listener.poll()
                } else {
                    Events::empty()
                }
            }
        }
    }

    pub fn register_observer(&self, observer: Arc<dyn Observer>, mask: Events) -> Result<()> {
        match &*self.inner() {
            Status::Connected(endpoint) => endpoint.register_observer(observer, mask),
            Status::Listening(addr) => {
                if let Some(listener) = ADDRESS_SPACE.get_listener_ref(addr) {
                    listener.register_observer(observer, mask)
                } else {
                    return_errno!(EINVAL, "can't find listener");
                }
            }
            _ => {
                return_errno!(EINVAL, "can't register observer");
            }
        }
        Ok(())
    }

    pub fn unregister_observer(&self, observer: &Arc<dyn Observer>) -> Result<Arc<dyn Observer>> {
        match &*self.inner() {
            Status::Connected(endpoint) => endpoint.unregister_observer(observer),
            Status::Listening(addr) => {
                if let Some(listener) = ADDRESS_SPACE.get_listener_ref(addr) {
                    listener.unregister_observer(observer)
                } else {
                    return_errno!(EINVAL, "can't find listener");
                }
            }
            _ => {
                return_errno!(EINVAL, "can't unregister observer");
            }
        }
    }

    pub fn ioctl(&self, cmd: &mut dyn IoctlCmd) -> Result<()> {
        async_io::match_ioctl_cmd_auto_error!(cmd, {
            cmd : GetReadBufLen => {
                match &*self.inner() {
                    Status::Connected(endpoint) => {
                        let read_buf_len = endpoint.bytes_to_read();
                        cmd.set_output(read_buf_len as _);
                    }
                    _ => {
                        cmd.set_output(0)
                    }
                }
            },
            cmd: SetNonBlocking => {
                self.set_nonblocking(*cmd.input() != 0); // 0 means blocking
            },
            cmd: SetRecvTimeoutCmd => {
                match &*self.inner() {
                    Status::Connected(endpoint) => {
                        endpoint.reader().set_receiver_timeout(*cmd.timeout());
                    }
                    _ => warn!("set recv timeout for other states not supported"),
                }
            },
            cmd: SetSendTimeoutCmd => {
                match &mut *self.inner() {
                    Status::Connected(endpoint) => {
                        endpoint.writer().set_sender_timeout(*cmd.timeout());
                    }
                    _ => warn!("set send timeout for other states not supported"),
                }
            },
            cmd: SetSockOptRawCmd => {
                // FIXME: Currently, it is harmless to ignore errors here.
                // When it is, implement this cmd or throw specific errors.
                warn!("setsockopt command has not been supported");
            }
        });
        Ok(())
    }

    pub fn socketpair(nonblocking: bool) -> Result<(Self, Self)> {
        let (end_a, end_b) = end_pair(nonblocking)?;

        let socket_a = Self {
            inner: SgxMutex::new(Status::Connected(end_a)),
        };

        let socket_b = Self {
            inner: SgxMutex::new(Status::Connected(end_b)),
        };

        Ok((socket_a, socket_b))
    }

    pub fn addr(&self) -> Result<TrustedAddr> {
        let addr = match &*self.inner() {
            Status::Idle(info) => info.addr().clone(),
            Status::Connected(endpoint) => endpoint.addr(),
            Status::Listening(addr) => Some(addr).cloned(),
        };

        return Ok(addr.unwrap_or_default());
    }

    pub fn domain(&self) -> Domain {
        Domain::Unix
    }

    pub fn peer_addr(&self) -> Result<TrustedAddr> {
        if let Status::Connected(endpoint) = &*self.inner() {
            if let Some(addr) = endpoint.peer_addr() {
                return Ok(addr);
            } else {
                return Ok(TrustedAddr::default());
            }
        }
        return_errno!(ENOTCONN, "the socket is not connected");
    }

    pub fn bind(&self, addr: &TrustedAddr) -> Result<()> {
        match &mut *self.inner() {
            Status::Idle(ref mut info) => {
                if info.addr().is_some() {
                    return_errno!(EINVAL, "the socket is already bound");
                }

                // check the global address space to see if the address is available before bind
                ADDRESS_SPACE.add_binder(addr)?;
                info.set_addr(addr);
            }
            Status::Connected(endpoint) => {
                if endpoint.addr().is_some() {
                    return_errno!(EINVAL, "the socket is already bound");
                }

                ADDRESS_SPACE.add_binder(addr)?;
                endpoint.set_addr(addr);
            }
            Status::Listening(_) => return_errno!(EINVAL, "the socket is already bound"),
        }
        Ok(())
    }

    pub fn listen(&self, backlog: u32) -> Result<()> {
        //TODO: restrict backlog according to /proc/sys/net/core/somaxconn
        let capacity = backlog as usize;

        let mut inner = self.inner();
        match &*inner {
            Status::Idle(info) => {
                if let Some(addr) = info.addr() {
                    debug!("listen addr = {:?}", addr);
                    ADDRESS_SPACE.add_listener(addr, capacity, info.nonblocking())?;
                    *inner = Status::Listening(addr.clone());
                } else {
                    return_errno!(EINVAL, "the socket is not bound");
                }
            }
            Status::Listening(addr) => {
                if let Some(listener) = ADDRESS_SPACE.get_listener_ref(addr) {
                    let nonblocking = listener.nonblocking();
                    ADDRESS_SPACE.add_listener(addr, capacity, nonblocking)?;
                } else {
                    return_errno!(EINVAL, "something wrong with listen");
                }
            }
            Status::Connected(_) => return_errno!(EINVAL, "the socket is already connected"),
        }

        Ok(())
    }

    /// The establishment of the connection is very fast and can be done immediately.
    /// Therefore, the connect function in our implementation will never block.
    pub async fn connect(&self, addr: &TrustedAddr) -> Result<()> {
        debug!("connect to {:?}", addr);

        let mut inner = self.inner();
        match &*inner {
            Status::Idle(info) => {
                let self_addr_opt = info.addr();
                if let Some(self_addr) = self_addr_opt {
                    if self_addr == addr {
                        return_errno!(EINVAL, "self connect is not supported");
                    }
                }

                let (end_self, end_incoming) = end_pair(info.nonblocking())?;
                end_incoming.set_addr(addr);
                if let Some(self_addr) = self_addr_opt {
                    end_self.set_addr(self_addr);
                }

                ADDRESS_SPACE
                    .push_incoming(addr, end_incoming)
                    .map_err(|e| match e.errno() {
                        Errno::EAGAIN => errno!(ECONNREFUSED, "the backlog is full"),
                        _ => e,
                    })?;

                *inner = Status::Connected(end_self);
                Ok(())
            }
            Status::Connected(endpoint) => return_errno!(EISCONN, "already connected"),
            Status::Listening(addr) => return_errno!(EINVAL, "invalid socket for connect"),
        }
    }

    // TODO: Support blocking accept. Current implementation is non blocking.
    pub async fn accept(&self, nonblocking: bool) -> Result<Self> {
        let status = (*self.inner()).clone();
        match status {
            Status::Listening(addr) => {
                debug!("accept addr = {:?}", addr);
                let mut endpoint = ADDRESS_SPACE.pop_incoming(&addr).await?;
                endpoint.set_nonblocking(nonblocking);
                let peer_addr = endpoint.peer_addr();

                debug!("accept socket from {:?}", peer_addr);

                Ok(Self {
                    inner: SgxMutex::new(Status::Connected(endpoint)),
                })
            }
            _ => return_errno!(EINVAL, "the socket is not listening"),
        }
    }

    pub async fn shutdown(&self, how: Shutdown) -> Result<()> {
        if let Status::Connected(ref end) = &*self.inner() {
            end.shutdown(how)
        } else {
            return_errno!(ENOTCONN, "The socket is not connected.");
        }
    }

    pub fn nonblocking(&self) -> bool {
        match &*self.inner() {
            Status::Idle(info) => info.nonblocking(),
            Status::Connected(endpoint) => endpoint.nonblocking(),
            Status::Listening(addr) => ADDRESS_SPACE.get_listener_ref(&addr).unwrap().nonblocking(),
        }
    }

    pub(super) fn set_nonblocking(&self, nonblocking: bool) {
        match &mut *self.inner() {
            Status::Idle(ref mut info) => info.set_nonblocking(nonblocking),
            Status::Connected(ref mut endpoint) => endpoint.set_nonblocking(nonblocking),
            Status::Listening(addr) => ADDRESS_SPACE
                .get_listener_ref(&addr)
                .unwrap()
                .set_nonblocking(nonblocking),
        }
    }
}

impl Debug for Stream {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Stream")
            .field("addr", &self.addr())
            .field("nonblocking", &self.nonblocking())
            .finish()
    }
}

impl Drop for Stream {
    fn drop(&mut self) {
        match &*self.inner() {
            Status::Idle(info) => {
                if let Some(addr) = info.addr() {
                    ADDRESS_SPACE.remove_addr(&addr);
                }
            }
            Status::Listening(addr) => {
                let listener = ADDRESS_SPACE.get_listener_ref(&addr).unwrap();
                ADDRESS_SPACE.remove_addr(&addr);
                // handle the blocking of other sockets holding the reference to the listener,
                // e.g., pushing to a listener full of incoming sockets
                listener.shutdown();
            }
            _ => {}
        }
    }
}

#[derive(Clone)]
pub enum Status {
    Idle(Info),
    // The listeners are stored in a global data structure indexed by the address.
    // The consistency of Status with that data structure should be carefully maintained.
    Listening(TrustedAddr),
    Connected(SockEnd),
}

#[derive(Debug, Clone)]
pub struct Info {
    addr: Option<TrustedAddr>,
    nonblocking: bool,
}

impl Info {
    pub fn new(nonblocking: bool) -> Self {
        Self {
            addr: None,
            nonblocking: nonblocking,
        }
    }

    pub fn addr(&self) -> &Option<TrustedAddr> {
        &self.addr
    }

    pub fn set_addr(&mut self, addr: &TrustedAddr) {
        self.addr = Some(addr.clone());
    }

    pub fn nonblocking(&self) -> bool {
        self.nonblocking
    }

    pub fn set_nonblocking(&mut self, nonblocking: bool) {
        self.nonblocking = nonblocking;
    }
}

/// The listener status of a stream unix socket.
/// It contains a channel holding incoming connections.
/// The nonblocking status of the reader end keeps the same with the socket.
/// The writer end is always non-blocking. The connect function returns
/// ECONNREFUSED rather than block when the channel is full.
pub struct Listener {
    channel: RwLock<Channel<SockEnd>>,
}

impl Listener {
    pub fn new(capacity: usize, nonblocking: bool) -> Result<Self> {
        let status_flag = {
            match nonblocking {
                true => StatusFlags::O_NONBLOCK,
                false => StatusFlags::empty(),
            }
        };
        let channel = Channel::with_capacity_and_flags(capacity, status_flag)?;

        Ok(Self {
            channel: RwLock::new(channel),
        })
    }

    pub fn capacity(&self) -> usize {
        let channel = self.channel.read().unwrap();
        channel.capacity()
    }

    pub fn push_incoming(&self, stream_socket: SockEnd) -> Result<()> {
        let channel = self.channel.read().unwrap();
        channel.push(stream_socket)
    }

    pub async fn pop_incoming(&self) -> Result<SockEnd> {
        let channel = self.channel.read().unwrap();
        channel.pop().await
    }

    pub fn nonblocking(&self) -> bool {
        let channel = self.channel.read().unwrap();
        channel
            .consumer()
            .status_flags()
            .contains(StatusFlags::O_NONBLOCK)
    }

    pub fn set_nonblocking(&self, nonblocking: bool) {
        let status_flag = {
            match nonblocking {
                true => StatusFlags::O_NONBLOCK,
                false => StatusFlags::empty(),
            }
        };
        let channel = self.channel.read().unwrap();
        channel.consumer().set_status_flags(status_flag);
    }

    pub fn shutdown(&self) {
        let channel = self.channel.read().unwrap();
        channel.consumer().shutdown();
    }

    pub fn poll(&self) -> Events {
        let mut events = Events::empty();
        let channel = self.channel.read().unwrap();
        let item_num = channel.items_to_consume();
        if item_num > 0 {
            events |= Events::IN;
        }
        events
    }

    pub fn register_observer(&self, observer: Arc<dyn Observer>, mask: Events) {
        let channel = self.channel.read().unwrap();
        channel
            .consumer()
            .register_observer(observer, mask)
            .unwrap()
    }

    pub fn unregister_observer(&self, observer: &Arc<dyn Observer>) -> Result<Arc<dyn Observer>> {
        let channel = self.channel.read().unwrap();
        channel.consumer().unregister_observer(observer)
    }
}
