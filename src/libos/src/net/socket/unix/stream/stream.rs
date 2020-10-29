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
            inner: SgxMutex::new(Status::Unconnected(Info::new(
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
            Status::Unconnected(info) => info.addr().clone(),
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
            Status::Unconnected(ref mut info) => {
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
            Status::Unconnected(info) => {
                if let Some(addr) = info.addr() {
                    ADDRESS_SPACE.add_listener(addr, capacity)?;
                    *inner = Status::Listening(addr.clone());
                } else {
                    return_errno!(EINVAL, "the socket is not bound");
                }
            }
            Status::Connected(_) => return_errno!(EINVAL, "the socket is already connected"),
            /// Modify the capacity of the channel holding incoming sockets
            Status::Listening(addr) => ADDRESS_SPACE.add_listener(&addr, capacity)?,
        }

        Ok(())
    }

    pub fn connect(&self, addr: &Addr) -> Result<()> {
        debug!("connect to {:?}", addr);

        let mut inner = self.inner();
        match &*inner {
            Status::Unconnected(info) => {
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

                ADDRESS_SPACE.push_incoming(addr, end_incoming)?;

                *inner = Status::Connected(end_self);
                Ok(())
            }
            Status::Connected(endpoint) => return_errno!(EISCONN, "already connected"),
            Status::Listening(addr) => return_errno!(EINVAL, "invalid socket for connect"),
        }
    }

    pub fn accept(&self, flags: FileFlags) -> Result<(Self, Option<Addr>)> {
        match &*self.inner() {
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
            Status::Unconnected(info) => info.nonblocking(),
            Status::Connected(endpoint) => endpoint.nonblocking(),
            Status::Listening(addr) => ADDRESS_SPACE.get_listener_ref(&addr).unwrap().nonblocking(),
        }
    }

    pub(super) fn set_nonblocking(&self, nonblocking: bool) {
        match &mut *self.inner() {
            Status::Unconnected(ref mut info) => info.set_nonblocking(nonblocking),
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
            Status::Unconnected(info) => {
                if let Some(addr) = info.addr() {
                    ADDRESS_SPACE.remove_addr(&addr);
                }
            }
            Status::Listening(addr) => {
                let listener = ADDRESS_SPACE.get_listener_ref(&addr).unwrap();
                ADDRESS_SPACE.remove_addr(&addr);
                /// handle the blocking of other sockets holding the reference to the listener,
                /// e.g., pushing to a listener full of incoming sockets
                listener.shutdown();
            }
            _ => {}
        }
    }
}

pub enum Status {
    Unconnected(Info),
    /// The listeners are stored in a global data structure indexed by the address.
    /// The consitency of Status with that data structure should be carefully maintained.
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

pub struct Listener {
    channel: Channel<Endpoint>,
    nonblocking: AtomicBool,
}

impl Listener {
    pub fn new(capacity: usize) -> Result<Self> {
        let channel = Channel::new(capacity)?;
        // It may incur blocking inside a blocking if the channel is blocking. Set the channel to
        // nonblocking permanently to avoid the nested blocking. This also results in nonblocking
        // accept and connect. Future work is needed to resolve this blocking issue to support
        // blocking accept and connect.
        channel.set_nonblocking(true);
        /// The listener is blocking by default
        let nonblocking = AtomicBool::new(true);

        Ok(Self {
            channel,
            nonblocking,
        })
    }

    pub fn push_incoming(&self, stream_socket: Endpoint) {
        self.channel.push(stream_socket);
    }

    pub fn pop_incoming(&self) -> Option<Endpoint> {
        self.channel.pop().ok().flatten()
    }

    pub fn remaining(&self) -> usize {
        self.channel.items_to_consume()
    }

    pub fn nonblocking(&self) -> bool {
        warn!("the channel works in a nonblocking way regardless of the nonblocking status");

        self.nonblocking.load(Ordering::Acquire)
    }

    pub fn set_nonblocking(&self, nonblocking: bool) {
        warn!("the channel works in a nonblocking way regardless of the nonblocking status");

        self.nonblocking.store(nonblocking, Ordering::Release);
    }

    pub fn shutdown(&self) {
        self.channel.shutdown();
    }
}
