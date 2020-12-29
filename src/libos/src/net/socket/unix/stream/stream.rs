use super::address_space::ADDRESS_SPACE;
use super::endpoint::{end_pair, Endpoint};
use super::*;
use alloc::sync::Arc;
use fs::channel::Channel;
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};

/// SOCK_STREAM Unix socket. It has three statuses: unconnected, listening and connected.  When a
/// socket is created, it is in unconnected status.  It will transfer to listening after listen is
/// called and connected after connect is called. A socket in connected status can be obtained
/// through a listening socket calling accept. Listening and connected are ultimate statuses. They
/// will not transfer to other statuses.
pub struct Stream {
    inner: SgxMutex<Status>,
}

impl Stream {
    pub fn new(flags: FileFlags) -> Self {
        Self {
            inner: SgxMutex::new(Status::Idle(Info::new(
                flags.contains(FileFlags::SOCK_NONBLOCK),
            ))),
        }
    }

    pub fn socketpair(flags: FileFlags) -> Result<(Self, Self)> {
        let nonblocking = flags.contains(FileFlags::SOCK_NONBLOCK);
        let (end_a, end_b) = end_pair(nonblocking)?;

        let socket_a = Self {
            inner: SgxMutex::new(Status::Connected(end_a)),
        };

        let socket_b = Self {
            inner: SgxMutex::new(Status::Connected(end_b)),
        };

        Ok((socket_a, socket_b))
    }

    pub fn addr(&self) -> Option<Addr> {
        match &*self.inner() {
            Status::Idle(info) => info.addr().clone(),
            Status::Connected(endpoint) => endpoint.addr(),
            Status::Listening(addr) => Some(addr).cloned(),
        }
    }

    pub fn peer_addr(&self) -> Result<Addr> {
        if let Status::Connected(endpoint) = &*self.inner() {
            if let Some(addr) = endpoint.peer_addr() {
                return Ok(addr);
            }
        }
        return_errno!(ENOTCONN, "the socket is not connected");
    }

    // TODO: create the corresponding file in the fs
    pub fn bind(&self, addr: &Addr) -> Result<()> {
        match &mut *self.inner() {
            Status::Idle(ref mut info) => {
                if info.addr().is_some() {
                    return_errno!(EINVAL, "the socket is already bound");
                }

                // check the global address space to see if the address is avaiable before bind
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

    pub fn listen(&self, backlog: i32) -> Result<()> {
        //TODO: restrict backlog accroding to /proc/sys/net/core/somaxconn
        if backlog < 0 {
            return_errno!(EINVAL, "negative backlog is not supported");
        }
        let capacity = backlog as usize;

        let mut inner = self.inner();
        match &*inner {
            Status::Idle(info) => {
                if let Some(addr) = info.addr() {
                    ADDRESS_SPACE.add_listener(addr, capacity, info.nonblocking())?;
                    *inner = Status::Listening(addr.clone());
                } else {
                    return_errno!(EINVAL, "the socket is not bound");
                }
            }
            Status::Connected(_) => return_errno!(EINVAL, "the socket is already connected"),
            // Modify the capacity of the channel holding incoming sockets
            Status::Listening(addr) => ADDRESS_SPACE.resize_listener(&addr, capacity)?,
        }

        Ok(())
    }

    /// The establishment of the connection is very fast and can be done immediately.
    /// Therefore, the connect function in our implementation will never block.
    pub fn connect(&self, addr: &Addr) -> Result<()> {
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

    pub fn accept(&self, flags: FileFlags) -> Result<(Self, Option<Addr>)> {
        let status = (*self.inner()).clone();
        match status {
            Status::Listening(addr) => {
                let endpoint = ADDRESS_SPACE.pop_incoming(&addr)?;
                endpoint.set_nonblocking(flags.contains(FileFlags::SOCK_NONBLOCK));

                let peer_addr = endpoint.peer_addr();

                debug!("accept socket from {:?}", peer_addr);

                Ok((
                    Self {
                        inner: SgxMutex::new(Status::Connected(endpoint)),
                    },
                    peer_addr,
                ))
            }
            _ => return_errno!(EINVAL, "the socket is not listening"),
        }
    }

    // TODO: handle flags
    pub fn sendto(&self, buf: &[u8], flags: SendFlags, addr: &Option<Addr>) -> Result<usize> {
        self.write(buf)
    }

    // TODO: handle flags
    pub fn recvfrom(&self, buf: &mut [u8], flags: RecvFlags) -> Result<(usize, Option<Addr>)> {
        let data_len = self.read(buf)?;
        let addr = self.peer_addr().ok();

        debug!("recvfrom {:?}", addr);

        Ok((data_len, addr))
    }

    /// perform shutdown on the socket.
    pub fn shutdown(&self, how: HowToShut) -> Result<()> {
        if let Status::Connected(ref end) = &*self.inner() {
            end.shutdown(how)
        } else {
            return_errno!(ENOTCONN, "The socket is not connected.");
        }
    }

    pub(super) fn nonblocking(&self) -> bool {
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

    pub(super) fn inner(&self) -> SgxMutexGuard<'_, Status> {
        self.inner.lock().unwrap()
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
    // The consitency of Status with that data structure should be carefully maintained.
    Listening(Addr),
    Connected(Endpoint),
}

#[derive(Debug, Clone)]
pub struct Info {
    addr: Option<Addr>,
    nonblocking: bool,
}

impl Info {
    pub fn new(nonblocking: bool) -> Self {
        Self {
            addr: None,
            nonblocking: nonblocking,
        }
    }

    pub fn addr(&self) -> &Option<Addr> {
        &self.addr
    }

    pub fn set_addr(&mut self, addr: &Addr) {
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
    channel: RwLock<Channel<Endpoint>>,
}

impl Listener {
    pub fn new(capacity: usize, nonblocking: bool) -> Result<Self> {
        let channel = Channel::new(capacity)?;
        channel.producer().set_nonblocking(true);
        channel.consumer().set_nonblocking(nonblocking);

        Ok(Self {
            channel: RwLock::new(channel),
        })
    }

    pub fn capacity(&self) -> usize {
        let channel = self.channel.read().unwrap();
        channel.capacity()
    }

    // TODO: when pop_incoming is blocked somewhere, the resize operation will blockingly wait for
    // the block to end. This is a rare scenario, so we will fix it in the future.
    pub fn resize(&self, capacity: usize) {
        if self.capacity() == capacity {
            return;
        }

        let mut channel = self.channel.write().unwrap();
        let new_channel = Channel::new(capacity).unwrap();
        new_channel.producer().set_nonblocking(true);
        new_channel
            .consumer()
            .set_nonblocking(channel.consumer().is_nonblocking());

        let remaining = channel.items_to_consume();
        for i in 0..std::cmp::min(remaining, capacity) {
            new_channel.push(channel.pop().unwrap().unwrap()).unwrap();
        }

        *channel = new_channel;
    }

    pub fn push_incoming(&self, stream_socket: Endpoint) -> Result<()> {
        let channel = self.channel.read().unwrap();
        channel.push(stream_socket)
    }

    pub fn pop_incoming(&self) -> Option<Endpoint> {
        let channel = self.channel.read().unwrap();
        channel.pop().ok().flatten()
    }

    pub fn remaining(&self) -> usize {
        let channel = self.channel.read().unwrap();
        channel.items_to_consume()
    }

    pub fn nonblocking(&self) -> bool {
        let channel = self.channel.read().unwrap();
        channel.consumer().is_nonblocking()
    }

    pub fn set_nonblocking(&self, nonblocking: bool) {
        let channel = self.channel.read().unwrap();
        channel.consumer().set_nonblocking(nonblocking);
    }

    pub fn shutdown(&self) {
        let channel = self.channel.read().unwrap();
        channel.shutdown();
    }
}
