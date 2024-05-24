use super::address_space::ADDRESS_SPACE;
use super::endpoint::{end_pair, Ancillary, Endpoint, RelayNotifier};
use super::*;
use events::{Event, EventFilter, Notifier, Observer};
use fs::channel::Channel;
use fs::IoEvents;
use fs::{CreationFlags, FileMode};
use net::socket::{CMessages, CmsgData};
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// SOCK_STREAM Unix socket. It has three statuses: unconnected, listening and connected.  When a
/// socket is created, it is in unconnected status.  It will transfer to listening after listen is
/// called and connected after connect is called. A socket in connected status can be obtained
/// through a listening socket calling accept. Listening and connected are ultimate statuses. They
/// will not transfer to other statuses.
pub struct Stream {
    inner: SgxMutex<Status>,
    // Use the internal notifier of RelayNotifier as the notifier of stream socket. It relays the
    // events of the endpoint, too.
    pub(super) notifier: Arc<RelayNotifier>,
}

impl Stream {
    pub fn new(flags: SocketFlags) -> Self {
        Self {
            inner: SgxMutex::new(Status::Idle(Info::new(
                flags.contains(SocketFlags::SOCK_NONBLOCK),
            ))),
            notifier: Arc::new(RelayNotifier::new()),
        }
    }

    pub fn socketpair(flags: SocketFlags) -> Result<(Self, Self)> {
        let nonblocking = flags.contains(SocketFlags::SOCK_NONBLOCK);
        let (end_a, end_b) = end_pair(nonblocking)?;
        let notifier_a = Arc::new(RelayNotifier::new());
        let notifier_b = Arc::new(RelayNotifier::new());
        notifier_a.observe_endpoint(&end_a);
        notifier_b.observe_endpoint(&end_b);

        let socket_a = Self {
            inner: SgxMutex::new(Status::Connected(end_a)),
            notifier: notifier_a,
        };

        let socket_b = Self {
            inner: SgxMutex::new(Status::Connected(end_b)),
            notifier: notifier_b,
        };

        Ok((socket_a, socket_b))
    }

    pub fn addr(&self) -> UnixAddr {
        let addr_opt = match &*self.inner() {
            Status::Idle(info) => info.addr().clone(),
            Status::Connected(endpoint) => endpoint.addr(),
            Status::Listening(addr) => Some(addr).cloned(),
        };

        addr_opt.unwrap_or(UnixAddr::Unnamed)
    }

    pub fn peer_addr(&self) -> Result<UnixAddr> {
        if let Status::Connected(endpoint) = &*self.inner() {
            if let Some(addr) = endpoint.peer_addr() {
                return Ok(addr);
            }
        }
        return_errno!(ENOTCONN, "the socket is not connected");
    }

    pub fn bind(&self, addr: &UnixAddr) -> Result<()> {
        let mut unix_addr = addr.clone();
        let addr = &mut unix_addr;

        if let UnixAddr::File(inode_num, path) = addr {
            // create the corresponding file in the fs and fill Addr with its inode
            let corresponding_inode_num = {
                let current = current!();
                let fs = current.fs().read().unwrap();
                let file_ref = fs.open_file(
                    path.path_str(),
                    CreationFlags::O_CREAT.bits(),
                    FileMode::from_bits(0o777).unwrap(),
                )?;
                file_ref.metadata()?.inode
            };
            *inode_num = Some(corresponding_inode_num);
        }

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
                    warn!("addr = {:?}", addr);
                    ADDRESS_SPACE.add_listener(
                        addr,
                        capacity,
                        info.nonblocking(),
                        self.notifier.clone(),
                    )?;
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
    pub fn connect(&self, addr: &UnixAddr) -> Result<()> {
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
                end_self.set_ancillary(Ancillary {
                    tid: current!().tid(),
                });

                ADDRESS_SPACE
                    .push_incoming(addr, end_incoming)
                    .map_err(|e| match e.errno() {
                        EAGAIN => errno!(ECONNREFUSED, "the backlog is full"),
                        _ => e,
                    })?;

                self.notifier.observe_endpoint(&end_self);

                // Notify listener for this event
                if let Some(listener) = ADDRESS_SPACE.get_listener_ref(addr) {
                    listener.notifier.notifier().broadcast(&IoEvents::IN);
                }

                *inner = Status::Connected(end_self);
                Ok(())
            }
            Status::Connected(endpoint) => return_errno!(EISCONN, "already connected"),
            Status::Listening(addr) => return_errno!(EINVAL, "invalid socket for connect"),
        }
    }

    pub fn accept(&self, flags: SocketFlags) -> Result<(Self, Option<UnixAddr>)> {
        let status = (*self.inner()).clone();
        match status {
            Status::Listening(addr) => {
                let endpoint = ADDRESS_SPACE.pop_incoming(&addr)?;
                endpoint.set_nonblocking(flags.contains(SocketFlags::SOCK_NONBLOCK));
                endpoint.set_ancillary(Ancillary {
                    tid: current!().tid(),
                });
                let notifier = Arc::new(RelayNotifier::new());
                notifier.observe_endpoint(&endpoint);

                let peer_addr = endpoint.peer_addr();

                debug!("accept socket from {:?}", peer_addr);

                Ok((
                    Self {
                        inner: SgxMutex::new(Status::Connected(endpoint)),
                        notifier: notifier,
                    },
                    peer_addr,
                ))
            }
            _ => return_errno!(EINVAL, "the socket is not listening"),
        }
    }

    // TODO: handle flags
    pub fn sendto(&self, buf: &[u8], flags: SendFlags, addr: Option<&UnixAddr>) -> Result<usize> {
        self.write(buf)
    }

    // TODO: handle flags
    pub fn recvfrom(&self, buf: &mut [u8], flags: RecvFlags) -> Result<(usize, Option<UnixAddr>)> {
        let data_len = self.read(buf)?;
        let addr = self.peer_addr().ok();

        debug!("recvfrom {:?}", addr);

        Ok((data_len, addr))
    }

    pub fn sendmsg(
        &self,
        bufs: &[&[u8]],
        flags: SendFlags,
        control: Option<&[u8]>,
    ) -> Result<usize> {
        if !flags.is_empty() {
            warn!("unsupported flags: {:?}", flags);
        }

        let data_len = self.writev(bufs)?;
        if let Some(msg_control) = control {
            self.write(msg_control)?;
        }

        Ok(data_len)
    }

    pub fn recvmsg(
        &self,
        bufs: &mut [&mut [u8]],
        flags: RecvFlags,
        control: Option<&mut [u8]>,
    ) -> Result<(usize, Option<AnyAddr>, MsgFlags, usize)> {
        if !flags.is_empty() {
            warn!("unsupported flags: {:?}", flags);
        }

        let data_len = self.readv(bufs)?;

        // For stream socket, the msg_name is ignored. And other fields are not supported.
        let control_len = if let Some(msg_control) = control {
            let control_len = self.read(msg_control)?;

            // For each control message that contains file descriptors (SOL_SOCKET and SCM_RIGHTS),
            // reassign each fd in the message in receive end.
            for cmsg in CMessages::from_bytes(msg_control) {
                if let CmsgData::ScmRights(mut scm_rights) = cmsg {
                    let send_tid = self.peer_ancillary().unwrap().tid();
                    scm_rights.iter_and_reassign_fds(|send_fd| {
                        let ipc_file = process::table::get_thread(send_tid)
                            .unwrap()
                            .files()
                            .lock()
                            .get(send_fd)
                            .unwrap();
                        current!().add_file(ipc_file.clone(), false)
                    })
                }
                // Unix credentials need not to be handled here
            }
            control_len
        } else {
            0
        };

        Ok((data_len, None, MsgFlags::empty(), control_len))
    }

    /// perform shutdown on the socket.
    pub fn shutdown(&self, how: Shutdown) -> Result<()> {
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

    fn ancillary(&self) -> Option<Ancillary> {
        match &*self.inner() {
            Status::Idle(_) => None,
            Status::Listening(_) => None,
            Status::Connected(endpoint) => endpoint.ancillary(),
        }
    }

    fn peer_ancillary(&self) -> Option<Ancillary> {
        if let Status::Connected(endpoint) = &*self.inner() {
            endpoint.peer_ancillary()
        } else {
            None
        }
    }

    fn set_ancillary(&self, ancillary: Ancillary) {
        if let Status::Connected(endpoint) = &*self.inner() {
            endpoint.set_ancillary(ancillary)
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
    // The consitency of Status with that data structure should be carefully maintained.
    Listening(UnixAddr),
    Connected(Endpoint),
}

#[derive(Debug, Clone)]
pub struct Info {
    addr: Option<UnixAddr>,
    nonblocking: bool,
}

impl Info {
    pub fn new(nonblocking: bool) -> Self {
        Self {
            addr: None,
            nonblocking: nonblocking,
        }
    }

    pub fn addr(&self) -> &Option<UnixAddr> {
        &self.addr
    }

    pub fn set_addr(&mut self, addr: &UnixAddr) {
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
    notifier: Arc<RelayNotifier>,
}

impl Listener {
    pub(super) fn new(
        capacity: usize,
        nonblocking: bool,
        notifier: Arc<RelayNotifier>,
    ) -> Result<Self> {
        let channel = Channel::new(capacity)?;
        channel.producer().set_nonblocking(true);
        channel.consumer().set_nonblocking(nonblocking);

        Ok(Self {
            channel: RwLock::new(channel),
            notifier,
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

    pub fn poll_new(&self) -> IoEvents {
        let mut events = IoEvents::empty();
        let channel = self.channel.read().unwrap();
        let item_num = channel.items_to_consume();
        if item_num > 0 {
            events |= IoEvents::IN;
        }
        events
    }
}
